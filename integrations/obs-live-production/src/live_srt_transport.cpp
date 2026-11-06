#include "live_srt_transport.hpp"
#include "live_address_plan.hpp"

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
    : settings_(std::move(settings)), runtime_ready_(srt_startup() >= 0) {}

LiveSrtTransport::~LiveSrtTransport() {
  interrupt();
  if (runtime_ready_) srt_cleanup();
}

bool LiveSrtTransport::connect() {
  if (!runtime_ready_ || interrupted_.load()) return false;
  const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(3);
  const auto addresses = resolve(settings_.host, lookup_id_, interrupted_);
  return connect_live_addresses(
      LiveAddressPlan{addresses, deadline}, std::chrono::steady_clock::now,
      [this](const QHostAddress &address, int budget) {
        return !interrupted_.load() && connect_address(address, budget);
      });
}

bool LiveSrtTransport::connect_address(const QHostAddress &address, int timeout_ms) {
  const SRTSOCKET socket = srt_create_socket();
  if (socket == SRT_INVALID_SOCK) return false;
  socket_.store(socket);
  const int yes = 1;
  const int timeout = 500;
  const int payload = 1316;
  const int transtype = SRTT_LIVE;
  const auto option = [&](SRT_SOCKOPT name, const void *value, int size) {
    return srt_setsockopt(socket, 0, name, value, size) != SRT_ERROR;
  };
  if (!option(SRTO_TRANSTYPE, &transtype, sizeof(transtype)) ||
      !option(SRTO_SNDTIMEO, &timeout, sizeof(timeout)) ||
      !option(SRTO_SENDER, &yes, sizeof(yes)) ||
      !option(SRTO_CONNTIMEO, &timeout_ms, sizeof(timeout_ms)) ||
      !option(SRTO_PAYLOADSIZE, &payload, sizeof(payload)) ||
      !option(SRTO_TSBPDMODE, &yes, sizeof(yes)) ||
      !option(SRTO_LATENCY, &settings_.latency_ms, sizeof(settings_.latency_ms))) {
    interrupt();
    return false;
  }
  const int key_length = 16;
  if (!option(SRTO_PBKEYLEN, &key_length, sizeof(key_length)) ||
      !option(SRTO_PASSPHRASE, settings_.passphrase.data(),
              static_cast<int>(settings_.passphrase.size())) ||
      (!settings_.stream_id.empty() &&
       !option(SRTO_STREAMID, settings_.stream_id.data(),
               static_cast<int>(settings_.stream_id.size())))) {
    interrupt();
    return false;
  }
  sockaddr_storage storage{};
  int length = 0;
  if (address.protocol() == QAbstractSocket::IPv4Protocol) {
    auto *value = reinterpret_cast<sockaddr_in *>(&storage);
    value->sin_family = AF_INET;
    value->sin_port = htons(settings_.port);
    value->sin_addr.s_addr = htonl(address.toIPv4Address());
    length = sizeof(sockaddr_in);
  } else {
    auto *value = reinterpret_cast<sockaddr_in6 *>(&storage);
    value->sin6_family = AF_INET6;
    value->sin6_port = htons(settings_.port);
    const Q_IPV6ADDR bytes = address.toIPv6Address();
    std::memcpy(&value->sin6_addr, bytes.c, sizeof(value->sin6_addr));
    length = sizeof(sockaddr_in6);
  }
  if (srt_connect(socket, reinterpret_cast<sockaddr *>(&storage), length) == SRT_ERROR) {
    SRTSOCKET expected = socket;
    if (socket_.compare_exchange_strong(expected, SRT_INVALID_SOCK)) srt_close(socket);
    return false;
  }
  if (interrupted_.load()) {
    interrupt();
    return false;
  }
  return true;
}

bool LiveSrtTransport::send(const uint8_t *data, size_t size) {
  const SRTSOCKET socket = socket_.load();
  if (socket == SRT_INVALID_SOCK || interrupted_.load()) return false;
  SRT_MSGCTRL control = srt_msgctrl_default;
  const int sent = srt_sendmsg2(socket, reinterpret_cast<const char *>(data),
                                static_cast<int>(size), &control);
  if (sent == static_cast<int>(size)) sent_bytes_.fetch_add(size);
  return sent == static_cast<int>(size);
}

void LiveSrtTransport::interrupt() {
  interrupted_.store(true);
  const int lookup = lookup_id_.exchange(-1);
  if (lookup >= 0) QHostInfo::abortHostLookup(lookup);
  const SRTSOCKET socket = socket_.exchange(SRT_INVALID_SOCK);
  if (socket != SRT_INVALID_SOCK) srt_close(socket);
}

}
