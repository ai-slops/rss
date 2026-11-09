#include "live_srt_transport.hpp"
#include "live_address_plan.hpp"
#include "live_srt_wire.hpp"

#include <arpa/inet.h>
#include <cerrno>
#include <csignal>
#include <dlfcn.h>
#include <filesystem>
#include <poll.h>
#include <spawn.h>
#include <sys/wait.h>
#include <thread>
#include <unistd.h>

extern char **environ;

#include <QCoreApplication>
#include <QEventLoop>
#include <QHostAddress>
#include <QHostInfo>
#include <QTimer>

#include <array>
#include <chrono>
#include <cstring>
#include <mutex>
#include <vector>

namespace live_production {
namespace {

std::vector<QHostAddress> resolve(const std::string &host,
                                  std::atomic<int> &lookup_id,
                                  std::atomic<bool> &interrupted) {
  QHostAddress literal(QString::fromStdString(host));
  if (!literal.isNull()) return {literal};
  if (!QCoreApplication::instance()) return {};
  QEventLoop loop;
  QTimer timer;
  QTimer cancel;
  timer.setSingleShot(true);
  std::vector<QHostAddress> addresses;
  QObject context;
  const int id = QHostInfo::lookupHost(
      QString::fromStdString(host), &context,
      [&](const QHostInfo &info) {
        if (!interrupted.load()) {
          const auto resolved = info.addresses();
          addresses.assign(resolved.begin(), resolved.end());
        }
        loop.quit();
      });
  lookup_id.store(id);
  QObject::connect(&timer, &QTimer::timeout, &loop, &QEventLoop::quit);
  QObject::connect(&cancel, &QTimer::timeout, &loop, [&] {
    if (interrupted.load()) loop.quit();
  });
  timer.start(3000);
  cancel.start(25);
  loop.exec();
  QHostInfo::abortHostLookup(id);
  lookup_id.store(-1);
  return addresses;
}

}

LiveSrtTransport::LiveSrtTransport(LiveSrtSettings settings)
    : settings_(std::move(settings)) {}

LiveSrtTransport::~LiveSrtTransport() { finish(); }

void LiveSrtTransport::finish() { interrupt(); close_session(); }

namespace {

std::filesystem::path helper_path() {
  Dl_info module{};
  if (!dladdr(reinterpret_cast<void *>(&helper_path), &module) || !module.dli_fname)
    return {};
  return std::filesystem::absolute(module.dli_fname).parent_path() / "obs-live-production-srt";
}

bool acknowledged(int fd, int timeout_ms) {
  pollfd descriptor{fd, POLLIN, 0};
  const auto deadline = std::chrono::steady_clock::now() + std::chrono::milliseconds(timeout_ms);
  while (std::chrono::steady_clock::now() < deadline) {
    const int remaining = static_cast<int>(std::chrono::duration_cast<std::chrono::milliseconds>(
        deadline - std::chrono::steady_clock::now()).count());
    if (poll(&descriptor, 1, remaining) < 0 && errno == EINTR) continue;
    char value = 0;
    return recv(fd, &value, 1, MSG_DONTWAIT) == 1 && value == 1;
  }
  return false;
}

}

bool LiveSrtTransport::connect() {
  if (interrupted_.load()) return false;
  const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(3);
  const auto addresses = resolve(settings_.host, lookup_id_, interrupted_);
  return connect_live_addresses(
      LiveAddressPlan{addresses, deadline}, std::chrono::steady_clock::now,
      [this](const QHostAddress &address, int budget) {
        return !interrupted_.load() && connect_address(address, budget);
      });
}

bool LiveSrtTransport::connect_address(const QHostAddress &address, int timeout_ms) {
  close_session();
  LiveSrtRequest request;
  request.timeout_ms = timeout_ms;
  request.latency_ms = settings_.latency_ms;
  if (settings_.passphrase.size() >= sizeof(request.passphrase) ||
      settings_.stream_id.size() >= sizeof(request.stream_id)) return false;
  std::memcpy(request.passphrase, settings_.passphrase.data(), settings_.passphrase.size());
  std::memcpy(request.stream_id, settings_.stream_id.data(), settings_.stream_id.size());
  if (address.protocol() == QAbstractSocket::IPv4Protocol) {
    auto *value = reinterpret_cast<sockaddr_in *>(&request.address);
    value->sin_family = AF_INET;
    value->sin_port = htons(settings_.port);
    value->sin_addr.s_addr = htonl(address.toIPv4Address());
    request.address_length = sizeof(*value);
  } else {
    auto *value = reinterpret_cast<sockaddr_in6 *>(&request.address);
    value->sin6_family = AF_INET6;
    value->sin6_port = htons(settings_.port);
    const Q_IPV6ADDR bytes = address.toIPv6Address();
    std::memcpy(&value->sin6_addr, bytes.c, sizeof(value->sin6_addr));
    request.address_length = sizeof(*value);
  }
  int sockets[2];
  if (socketpair(AF_UNIX, SOCK_SEQPACKET | SOCK_CLOEXEC | SOCK_NONBLOCK, 0, sockets)) return false;
  posix_spawn_file_actions_t actions;
  posix_spawn_file_actions_init(&actions);
  posix_spawn_file_actions_adddup2(&actions, sockets[1], STDIN_FILENO);
  const auto path = helper_path().string();
  char *arguments[] = {const_cast<char *>(path.c_str()), nullptr};
  pid_t child = -1;
  posix_spawnattr_t attributes;
  posix_spawnattr_init(&attributes);
  sigset_t empty, defaults;
  sigemptyset(&empty);
  sigemptyset(&defaults);
  sigaddset(&defaults, SIGTERM);
  sigaddset(&defaults, SIGINT);
  posix_spawnattr_setsigmask(&attributes, &empty);
  posix_spawnattr_setsigdefault(&attributes, &defaults);
  posix_spawnattr_setflags(&attributes, POSIX_SPAWN_SETSIGMASK | POSIX_SPAWN_SETSIGDEF);
  const int spawned = posix_spawn(&child, path.c_str(), &actions, &attributes, arguments, environ);
  posix_spawnattr_destroy(&attributes);
  posix_spawn_file_actions_destroy(&actions);
  ::close(sockets[1]);
  if (spawned != 0) { ::close(sockets[0]); return false; }
  {
    std::lock_guard<std::mutex> lock(session_mutex_);
    child_.store(child);
    ipc_.store(sockets[0]);
  }
  if (interrupted_.load()) { interrupt(); return false; }
  if (::send(sockets[0], &request, sizeof(request), MSG_NOSIGNAL) != sizeof(request) ||
      !acknowledged(sockets[0], timeout_ms)) {
    close_session();
    return false;
  }
  return !interrupted_.load();
}

bool LiveSrtTransport::send(const uint8_t *data, size_t size) {
  const int fd = ipc_.load();
  if (fd < 0 || interrupted_.load() || size > 1316) return false;
  if (::send(fd, data, size, MSG_NOSIGNAL) != static_cast<ssize_t>(size) ||
      !acknowledged(fd, 500)) return false;
  sent_bytes_.fetch_add(size);
  return true;
}

void LiveSrtTransport::interrupt() {
  interrupted_.store(true);
  const int lookup = lookup_id_.exchange(-1);
  if (lookup >= 0) QHostInfo::abortHostLookup(lookup);
  std::lock_guard<std::mutex> lock(session_mutex_);
  if (ipc_.load() >= 0) shutdown(ipc_.load(), SHUT_RDWR);
}

void LiveSrtTransport::close_session() {
  int child;
  {
    std::lock_guard<std::mutex> lock(session_mutex_);
    const int fd = ipc_.exchange(-1);
    if (fd >= 0) { shutdown(fd, SHUT_RDWR); ::close(fd); }
    child = child_.exchange(-1);
  }
  if (child <= 0) return;
  const auto deadline = std::chrono::steady_clock::now() + std::chrono::milliseconds(500);
  int status;
  bool terminated = false;
  while (std::chrono::steady_clock::now() < deadline) {
    const int result = waitpid(child, &status, WNOHANG);
    if (result == child || (result < 0 && errno == ECHILD)) return;
    if (!terminated && deadline - std::chrono::steady_clock::now() < std::chrono::milliseconds(100)) {
      kill(child, SIGTERM);
      terminated = true;
    }
    std::this_thread::sleep_for(std::chrono::milliseconds(10));
  }
  kill(child, SIGKILL);
  while (waitpid(child, &status, 0) < 0 && errno == EINTR) {}
}

}
