#pragma once

#include "live_settings.hpp"

#include <atomic>
#include <cstdint>
#include <string>

#include <mutex>
#include <QHostAddress>

namespace live_production {

class LiveSrtTransport {
 public:
  explicit LiveSrtTransport(LiveSrtSettings settings);
  ~LiveSrtTransport();
  bool connect();
  bool send(const uint8_t *data, size_t size);
  void interrupt();
  void finish();
  uint64_t sent_bytes() const { return sent_bytes_.load(); }

 private:
  bool connect_address(const QHostAddress &address, int timeout_ms);
  LiveSrtSettings settings_;
  void close_session();
  std::mutex session_mutex_;
  std::atomic<int> ipc_{-1};
  std::atomic<int> child_{-1};
  std::atomic<uint64_t> sent_bytes_{0};
  std::atomic<bool> interrupted_{false};
  std::atomic<int> lookup_id_{-1};
};

}
