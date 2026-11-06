#include "publisher_transport_fault.hpp"

#include <cerrno>
#include <cstdint>
#include <filesystem>
#include <fstream>
#include <optional>
#include <poll.h>
#include <set>
#include <sstream>
#include <string>
#include <vector>

#include <signal.h>
#include <sys/syscall.h>
#include <sys/types.h>
#include <unistd.h>

namespace live_production {
namespace {

constexpr const char *transport_executable = "obs-ffmpeg-mux";

struct Identity {
  pid_t pid = -1;
  uint64_t start_ticks = 0;

  bool operator<(const Identity &other) const {
    return pid < other.pid ||
           (pid == other.pid && start_ticks < other.start_ticks);
  }

  bool operator==(const Identity &other) const {
    return pid == other.pid && start_ticks == other.start_ticks;
  }
};

struct FrozenChild {
  Identity identity;
  int pidfd = -1;
};

std::optional<uint64_t> start_ticks(pid_t pid) {
  std::ifstream input("/proc/" + std::to_string(pid) + "/stat");
  std::string line;
  if (!std::getline(input, line)) return std::nullopt;
  const size_t end_name = line.rfind(')');
  if (end_name == std::string::npos || end_name + 2 >= line.size())
    return std::nullopt;
  std::istringstream fields(line.substr(end_name + 2));
  std::string value;
  for (int field = 3; field <= 22; ++field) {
    if (!(fields >> value)) return std::nullopt;
    if (field == 22) {
      try {
        return std::stoull(value);
      } catch (...) {
        return std::nullopt;
      }
    }
  }
  return std::nullopt;
}

std::optional<pid_t> parent_pid(pid_t pid) {
  std::ifstream input("/proc/" + std::to_string(pid) + "/status");
  std::string key;
  while (input >> key) {
    if (key == "PPid:") {
      pid_t parent = -1;
      if (input >> parent) return parent;
      return std::nullopt;
    }
    std::string ignored;
    std::getline(input, ignored);
  }
  return std::nullopt;
}

std::optional<std::string> executable_name(pid_t pid) {
  std::error_code error;
  const auto link = std::filesystem::read_symlink(
      "/proc/" + std::to_string(pid) + "/exe", error);
  if (error) return std::nullopt;
  return link.filename().string();
}

std::set<pid_t> child_pids() {
  std::set<pid_t> children;
  std::error_code error;
  std::filesystem::directory_iterator task("/proc/self/task", error);
  const std::filesystem::directory_iterator end;
  while (!error && task != end) {
    std::ifstream input(task->path() / "children");
    pid_t child = -1;
    while (input >> child) {
      if (child > 0) children.insert(child);
    }
    task.increment(error);
  }
  return children;
}

std::optional<Identity> owned_transport(pid_t pid,
                                        const std::set<pid_t> &children) {
  if (children.find(pid) == children.end()) return std::nullopt;
  const auto parent = parent_pid(pid);
  const auto executable = executable_name(pid);
  const auto generation = start_ticks(pid);
  if (!parent || *parent != ::getpid() || !executable ||
      *executable != transport_executable || !generation)
    return std::nullopt;
  return Identity{pid, *generation};
}

int open_pidfd(pid_t pid) {
#if defined(SYS_pidfd_open)
  return static_cast<int>(::syscall(SYS_pidfd_open, pid, 0));
#else
  (void)pid;
  errno = ENOSYS;
  return -1;
#endif
}

bool pidfd_alive(int pidfd) {
  pollfd descriptor{pidfd, POLLIN, 0};
  const int result = ::poll(&descriptor, 1, 0);
  return result == 0;
}

bool pidfd_signal(int pidfd, int signal) {
#if defined(SYS_pidfd_send_signal)
  return ::syscall(SYS_pidfd_send_signal, pidfd, signal, nullptr, 0) == 0;
#else
  (void)pidfd;
  (void)signal;
  errno = ENOSYS;
  return false;
#endif
}

std::set<Identity> baseline_children() {
  std::set<Identity> baseline;
  for (pid_t pid : child_pids()) {
    const auto generation = start_ticks(pid);
    if (generation) baseline.insert(Identity{pid, *generation});
  }
  return baseline;
}

}

class PublisherTransportFault::Impl {
 public:
  Impl() : baseline_(baseline_children()) {}

  ~Impl() { clear(); }

  bool freeze() {
    if (frozen_) return false;
    const auto children = child_pids();
    std::vector<Identity> candidates;
    for (pid_t pid : children) {
      const auto candidate = owned_transport(pid, children);
      if (candidate && baseline_.find(*candidate) == baseline_.end())
        candidates.push_back(*candidate);
    }
    if (candidates.size() != 1) return false;
    const int pidfd = open_pidfd(candidates.front().pid);
    if (pidfd < 0) return false;
    const auto confirmed = owned_transport(candidates.front().pid, child_pids());
    if (!confirmed || !(*confirmed == candidates.front()) || !pidfd_alive(pidfd) ||
        !pidfd_signal(pidfd, SIGSTOP)) {
      ::close(pidfd);
      return false;
    }
    frozen_ = FrozenChild{*confirmed, pidfd};
    return true;
  }

  bool resume() {
    if (!frozen_) return false;
    const int pidfd = frozen_->pidfd;
    const bool resumed = pidfd_signal(pidfd, SIGCONT);
    if (!resumed && errno != ESRCH) return false;
    ::close(pidfd);
    frozen_.reset();
    return resumed;
  }

  bool frozen() const { return frozen_.has_value(); }

 private:
  void clear() {
    if (frozen_) {
      (void)pidfd_signal(frozen_->pidfd, SIGCONT);
      ::close(frozen_->pidfd);
      frozen_.reset();
    }
  }

  std::set<Identity> baseline_;
  std::optional<FrozenChild> frozen_;
};

PublisherTransportFault::PublisherTransportFault()
    : impl_(std::make_unique<Impl>()) {}

PublisherTransportFault::~PublisherTransportFault() {
  (void)resume();
}

bool PublisherTransportFault::freeze() { return impl_->freeze(); }

bool PublisherTransportFault::resume() { return impl_->resume(); }

bool PublisherTransportFault::frozen() const { return impl_->frozen(); }

}
