#include "live_srt_transport.hpp"
#include "live_address_plan.hpp"
#include "live_srt_wire.hpp"

#include <windows.h>
#include <QCoreApplication>
#include <QEventLoop>
#include <QHostInfo>
#include <QProcess>
#include <QTimer>

#include <chrono>
#include <cstring>
#include <filesystem>
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
  QTimer timer, cancel;
  timer.setSingleShot(true);
  std::vector<QHostAddress> addresses;
  QObject context;
  const int id = QHostInfo::lookupHost(QString::fromStdString(host), &context,
      [&](const QHostInfo &info) {
        if (!interrupted.load()) {
          const auto values = info.addresses();
          addresses.assign(values.begin(), values.end());
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

QString helper_path() {
  HMODULE module = nullptr;
  if (!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS |
                         GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                         reinterpret_cast<LPCWSTR>(&helper_path), &module)) return {};
  std::vector<wchar_t> path(32768);
  const DWORD length = GetModuleFileNameW(module, path.data(), static_cast<DWORD>(path.size()));
  if (!length || length >= path.size()) return {};
  return QString::fromStdWString((std::filesystem::path(path.data()).parent_path() /
                                 L"obs-live-production-srt.exe").wstring());
}

}

class LiveSrtTransport::WindowsSession {
 public:
  QProcess process;
  HANDLE job = nullptr;
  ~WindowsSession() {
    process.closeWriteChannel();
    if (!process.waitForFinished(400)) {
      process.kill();
      process.waitForFinished(1000);
    }
    if (job) CloseHandle(job);
  }
  bool own_child() {
    job = CreateJobObjectW(nullptr, nullptr);
    if (!job) return false;
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION limits{};
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    if (!SetInformationJobObject(job, JobObjectExtendedLimitInformation, &limits,
                                 sizeof(limits))) return false;
    HANDLE child = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, FALSE,
                               static_cast<DWORD>(process.processId()));
    if (!child) return false;
    const bool assigned = AssignProcessToJobObject(job, child) != FALSE;
    CloseHandle(child);
    return assigned;
  }
  bool acknowledged(int timeout, const std::atomic<bool> &interrupted) {
    const auto deadline = std::chrono::steady_clock::now() + std::chrono::milliseconds(timeout);
    while (!interrupted.load() && std::chrono::steady_clock::now() < deadline) {
      if (process.bytesAvailable() > 0) return process.read(1) == QByteArray(1, char(1));
      if (process.state() == QProcess::NotRunning) return false;
      process.waitForReadyRead(10);
    }
    return false;
  }
};

LiveSrtTransport::LiveSrtTransport(LiveSrtSettings settings) : settings_(std::move(settings)) {}
LiveSrtTransport::~LiveSrtTransport() { finish(); }
void LiveSrtTransport::finish() { interrupt(); close_session(); }
void LiveSrtTransport::close_session() { windows_.reset(); }
void LiveSrtTransport::interrupt() {
  interrupted_.store(true);
  const int lookup = lookup_id_.exchange(-1);
  if (lookup >= 0) QHostInfo::abortHostLookup(lookup);
}

bool LiveSrtTransport::connect() {
  if (interrupted_.load()) return false;
  const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(3);
  const auto addresses = resolve(settings_.host, lookup_id_, interrupted_);
  return connect_live_addresses(LiveAddressPlan{addresses, deadline},
      std::chrono::steady_clock::now, [this](const QHostAddress &address, int budget) {
        return !interrupted_.load() && connect_address(address, budget);
      });
}

bool LiveSrtTransport::connect_address(const QHostAddress &address, int timeout_ms) {
  close_session();
  const auto deadline = std::chrono::steady_clock::now() + std::chrono::milliseconds(timeout_ms);
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
    const auto bytes = address.toIPv6Address();
    std::memcpy(&value->sin6_addr, bytes.c, sizeof(value->sin6_addr));
    request.address_length = sizeof(*value);
  }
  auto session = std::make_unique<WindowsSession>();
  session->process.setProgram(helper_path());
  session->process.setStandardErrorFile(QProcess::nullDevice());
  session->process.setCreateProcessArgumentsModifier([](QProcess::CreateProcessArguments *args) {
    args->flags |= CREATE_NO_WINDOW;
  });
  session->process.start();
  if (!session->process.waitForStarted(timeout_ms) || !session->own_child() || interrupted_.load())
    return false;
  const int remaining = static_cast<int>(std::chrono::duration_cast<std::chrono::milliseconds>(
      deadline - std::chrono::steady_clock::now()).count());
  if (remaining <= 0) return false;
  request.timeout_ms = remaining;
  if (session->process.write(reinterpret_cast<const char *>(&request), sizeof(request)) != sizeof(request) ||
      !session->acknowledged(remaining, interrupted_)) return false;
  windows_ = std::move(session);
  return true;
}

bool LiveSrtTransport::send(const uint8_t *data, size_t size) {
  if (!windows_ || interrupted_.load() || !size || size > 1316) return false;
  const uint32_t length = static_cast<uint32_t>(size);
  if (windows_->process.write(reinterpret_cast<const char *>(&length), sizeof(length)) != sizeof(length) ||
      windows_->process.write(reinterpret_cast<const char *>(data), static_cast<qint64>(size)) !=
          static_cast<qint64>(size) || !windows_->acknowledged(500, interrupted_)) return false;
  sent_bytes_.fetch_add(size);
  return true;
}

}
