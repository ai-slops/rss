#include "live_test_receiver.hpp"

#include <array>
#include <cstring>

#include <arpa/inet.h>
#include <netinet/in.h>

LiveTestReceiver::LiveTestReceiver() { srt_startup(); }

LiveTestReceiver::~LiveTestReceiver() { stop(); srt_cleanup(); }

bool LiveTestReceiver::start() {
  listener_ = srt_create_socket();
  if (listener_ == SRT_INVALID_SOCK) return false;
  const int yes = 1;
  const int timeout = 200;
  const int latency = 120;
  const int key_length = 16;
  const char passphrase[] = "fixtureCredential42";
  srt_setsockopt(listener_, 0, SRTO_RCVSYN, &yes, sizeof(yes));
  srt_setsockopt(listener_, 0, SRTO_RCVTIMEO, &timeout, sizeof(timeout));
  srt_setsockopt(listener_, 0, SRTO_LATENCY, &latency, sizeof(latency));
  srt_setsockopt(listener_, 0, SRTO_PBKEYLEN, &key_length, sizeof(key_length));
  srt_setsockopt(listener_, 0, SRTO_PASSPHRASE, passphrase,
                 static_cast<int>(std::strlen(passphrase)));
  sockaddr_in address{};
  address.sin_family = AF_INET;
  address.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
  if (srt_bind(listener_, reinterpret_cast<sockaddr *>(&address), sizeof(address)) ==
      SRT_ERROR)
    return false;
  int length = sizeof(address);
  if (srt_getsockname(listener_, reinterpret_cast<sockaddr *>(&address), &length) ==
          SRT_ERROR ||
      srt_listen(listener_, 1) == SRT_ERROR)
    return false;
  port_ = ntohs(address.sin_port);
  worker_ = std::thread([this] { run(); });
  return true;
}

void LiveTestReceiver::run() {
  std::array<char, 2048> buffer{};
  while (!stopped_.load()) {
    sockaddr_storage address{};
    int length = sizeof(address);
    const SRTSOCKET peer = srt_accept(listener_.load(), reinterpret_cast<sockaddr *>(&address), &length);
    if (peer == SRT_INVALID_SOCK) continue;
    peer_.store(peer);
    {
      std::lock_guard<std::mutex> lock(data_mutex_);
      captures_.emplace_back();
    }
    while (!stopped_.load()) {
      const int received = srt_recvmsg(peer, buffer.data(), static_cast<int>(buffer.size()));
      if (received <= 0) {
        if (srt_getsockstate(peer) == SRTS_BROKEN ||
            srt_getsockstate(peer) == SRTS_CLOSED)
          break;
        continue;
      }
      bytes_.fetch_add(static_cast<uint64_t>(received));
      packets_.fetch_add(1);
      std::lock_guard<std::mutex> lock(data_mutex_);
      data_.insert(data_.end(), buffer.begin(), buffer.begin() + received);
      captures_.back().insert(captures_.back().end(), buffer.begin(), buffer.begin() + received);
    }
    SRTSOCKET expected = peer;
    if (peer_.compare_exchange_strong(expected, SRT_INVALID_SOCK)) srt_close(peer);
    {
      std::lock_guard<std::mutex> lock(data_mutex_);
      ++completed_;
      completed_changed_.notify_all();
    }
  }
}

std::vector<uint8_t> LiveTestReceiver::data() const {
  std::lock_guard<std::mutex> lock(data_mutex_);
  return data_;
}

std::vector<std::vector<uint8_t>> LiveTestReceiver::captures() const {
  std::lock_guard<std::mutex> lock(data_mutex_);
  return captures_;
}

void LiveTestReceiver::stop() {
  stopped_.store(true);
  const auto peer = peer_.exchange(SRT_INVALID_SOCK);
  const auto listener = listener_.exchange(SRT_INVALID_SOCK);
  if (peer != SRT_INVALID_SOCK) srt_close(peer);
  if (listener != SRT_INVALID_SOCK) srt_close(listener);
  if (worker_.joinable()) worker_.join();
}

bool LiveTestReceiver::wait_completed(size_t count, std::chrono::milliseconds timeout) {
  std::unique_lock<std::mutex> lock(data_mutex_);
  return completed_changed_.wait_for(lock, timeout, [&] { return completed_ >= count; });
}
