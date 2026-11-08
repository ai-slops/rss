#include "publisher_transport_fault.hpp"

#include <atomic>
#include <chrono>
#include <csignal>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <stdexcept>
#include <string>
#include <thread>

#include <sys/stat.h>
#include <sys/prctl.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

namespace {

using live_production::PublisherTransportFault;
using namespace std::chrono_literals;

void require(bool value, const char *message) {
  if (!value) throw std::runtime_error(message);
}

class Child {
 public:
  Child() = default;
  explicit Child(pid_t pid) : pid_(pid) {}
  Child(const Child &) = delete;
  Child &operator=(const Child &) = delete;
  Child(Child &&other) noexcept : pid_(other.pid_) { other.pid_ = -1; }
  Child &operator=(Child &&other) noexcept {
    if (this != &other) {
      stop();
      pid_ = other.pid_;
      other.pid_ = -1;
    }
    return *this;
  }
  ~Child() { stop(); }

  pid_t pid() const { return pid_; }

  void kill_and_reap() {
    if (pid_ <= 0) return;
    (void)::kill(pid_, SIGKILL);
    (void)::waitpid(pid_, nullptr, 0);
    pid_ = -1;
  }

 private:
  void stop() {
    if (pid_ <= 0) return;
    (void)::kill(pid_, SIGCONT);
    (void)::kill(pid_, SIGTERM);
    for (int attempt = 0; attempt < 50; ++attempt) {
      if (::waitpid(pid_, nullptr, WNOHANG) == pid_) {
        pid_ = -1;
        return;
      }
      std::this_thread::sleep_for(10ms);
    }
    kill_and_reap();
  }

  pid_t pid_ = -1;
};

class TransportExecutable {
 public:
  TransportExecutable() {
    char pattern[] = "/tmp/publisher-transport-fault-XXXXXX";
    const char *created = ::mkdtemp(pattern);
    if (!created) throw std::runtime_error("mkdtemp failed");
    directory_ = created;
    path_ = directory_ / "obs-ffmpeg-mux";
    std::filesystem::copy_file("/proc/self/exe", path_);
    std::filesystem::permissions(
        path_, std::filesystem::perms::owner_all,
        std::filesystem::perm_options::replace);
  }
  ~TransportExecutable() { std::filesystem::remove_all(directory_); }
  const std::filesystem::path &path() const { return path_; }

 private:
  std::filesystem::path directory_;
  std::filesystem::path path_;
};

Child launch(const std::filesystem::path &path, const char *argument,
             const char *extra = nullptr) {
  const pid_t pid = ::fork();
  if (pid < 0) throw std::runtime_error("fork failed");
  if (pid == 0) {
    if (extra)
      ::execl(path.c_str(), path.c_str(), argument, extra, nullptr);
    else
      ::execl(path.c_str(), path.c_str(), argument, nullptr);
    _exit(127);
  }
  return Child(pid);
}

bool stopped(pid_t pid) {
  std::ifstream status("/proc/" + std::to_string(pid) + "/status");
  std::string line;
  while (std::getline(status, line)) {
    if (line.rfind("State:", 0) != 0) continue;
    const size_t value = line.find_first_not_of(" \t", line.find(':') + 1);
    return value != std::string::npos &&
           (line[value] == 'T' || line[value] == 't');
  }
  return false;
}

bool wait_state(pid_t pid, bool expected) {
  for (int attempt = 0; attempt < 100; ++attempt) {
    if (stopped(pid) == expected) return true;
    std::this_thread::sleep_for(10ms);
  }
  return false;
}

bool wait_executable(pid_t pid, const std::string &name) {
  for (int attempt = 0; attempt < 100; ++attempt) {
    std::error_code error;
    const auto executable = std::filesystem::read_symlink(
        "/proc/" + std::to_string(pid) + "/exe", error);
    if (!error && executable.filename() == name) return true;
    std::this_thread::sleep_for(10ms);
  }
  return false;
}

pid_t wait_first_child(pid_t parent) {
  for (int attempt = 0; attempt < 100; ++attempt) {
    std::ifstream children("/proc/" + std::to_string(parent) +
                           "/task/" + std::to_string(parent) + "/children");
    pid_t child = -1;
    if (children >> child) return child;
    std::this_thread::sleep_for(10ms);
  }
  return -1;
}

void owned_child_freezes_without_stopping_parent(
    const TransportExecutable &transport) {
  PublisherTransportFault fault;
  Child child;
  std::atomic<bool> ready{false};
  std::atomic<bool> release{false};
  std::exception_ptr creator_error;
  std::thread creator([&] {
    try {
      child = launch(transport.path(), "--transport-child");
    } catch (...) {
      creator_error = std::current_exception();
    }
    ready.store(true, std::memory_order_release);
    while (!release.load(std::memory_order_acquire)) std::this_thread::yield();
  });
  while (!ready.load(std::memory_order_acquire)) std::this_thread::yield();
  const bool executed = wait_executable(child.pid(), "obs-ffmpeg-mux");
  const bool frozen = executed && fault.freeze();
  release.store(true, std::memory_order_release);
  creator.join();
  if (creator_error) std::rethrow_exception(creator_error);
  require(executed, "transport helper did not exec");
  require(frozen, "new owned transport was not frozen");
  require(wait_state(child.pid(), true), "transport did not stop");
  require(::kill(::getpid(), 0) == 0, "publisher parent was not alive");
  require(fault.resume(), "owned transport did not resume");
  require(wait_state(child.pid(), false), "transport remained stopped");
}

void destructor_resumes_owned_child(const TransportExecutable &transport) {
  Child child;
  {
    PublisherTransportFault fault;
    child = launch(transport.path(), "--transport-child");
    require(wait_executable(child.pid(), "obs-ffmpeg-mux"),
            "destructor helper did not exec");
    require(fault.freeze(), "destructor case did not freeze");
    require(wait_state(child.pid(), true), "destructor case was not stopped");
    require(std::filesystem::remove(transport.path()), "cannot unlink owned executable");
  }
  std::filesystem::copy_file("/proc/self/exe", transport.path());
  require(wait_state(child.pid(), false), "destructor left transport stopped");
}

void baseline_foreign_and_ambiguous_children_are_rejected(
    const TransportExecutable &transport) {
  Child baseline = launch(transport.path(), "--transport-child");
  PublisherTransportFault baseline_fault;
  require(!baseline_fault.freeze(), "baseline transport was selected");

  PublisherTransportFault foreign_fault;
  Child foreign = launch("/bin/sleep", "30");
  require(!foreign_fault.freeze(), "foreign child was selected");

  PublisherTransportFault nested_fault;
  Child nesting_parent = launch("/proc/self/exe", "--foreign-parent",
                                transport.path().c_str());
  const pid_t nested = wait_first_child(nesting_parent.pid());
  require(nested > 0 && wait_executable(nested, "obs-ffmpeg-mux"),
          "nested transport helper did not exec");
  require(!nested_fault.freeze(), "nested foreign transport was selected");

  PublisherTransportFault ambiguous_fault;
  Child first = launch(transport.path(), "--transport-child");
  Child second = launch(transport.path(), "--transport-child");
  require(wait_executable(first.pid(), "obs-ffmpeg-mux"),
          "first ambiguous helper did not exec");
  require(wait_executable(second.pid(), "obs-ffmpeg-mux"),
          "second ambiguous helper did not exec");
  require(!ambiguous_fault.freeze(), "ambiguous transports were selected");
}

void stale_generation_cannot_signal_replacement(
    const TransportExecutable &transport) {
  PublisherTransportFault fault;
  Child original = launch(transport.path(), "--transport-child");
  require(wait_executable(original.pid(), "obs-ffmpeg-mux"),
          "generation helper did not exec");
  require(fault.freeze(), "generation case did not freeze");
  require(wait_state(original.pid(), true), "generation case was not stopped");
  original.kill_and_reap();
  Child replacement = launch(transport.path(), "--transport-child");
  require(wait_executable(replacement.pid(), "obs-ffmpeg-mux"),
          "replacement helper did not exec");
  require(!fault.resume(), "stale generation resumed a replacement");
  require(wait_state(replacement.pid(), false), "replacement was signalled");
  require(fault.freeze(), "replacement could not become a fresh candidate");
  require(fault.resume(), "replacement cleanup resume failed");
}

int child_main() {
  for (;;) ::pause();
}

int foreign_parent_main(const char *transport) {
  const pid_t child = ::fork();
  if (child < 0) return 2;
  if (child == 0) {
    (void)::prctl(PR_SET_PDEATHSIG, SIGTERM);
    ::execl(transport, transport, "--transport-child", nullptr);
    _exit(127);
  }
  return ::waitpid(child, nullptr, 0) < 0 ? 3 : 0;
}

}

int main(int argc, char **argv) {
  if (argc == 2 && std::string(argv[1]) == "--transport-child")
    return child_main();
  if (argc == 3 && std::string(argv[1]) == "--foreign-parent")
    return foreign_parent_main(argv[2]);
  try {
    const TransportExecutable transport;
    owned_child_freezes_without_stopping_parent(transport);
    destructor_resumes_owned_child(transport);
    baseline_foreign_and_ambiguous_children_are_rejected(transport);
    stale_generation_cannot_signal_replacement(transport);
    std::cout << "publisher transport fault tests passed\n";
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
