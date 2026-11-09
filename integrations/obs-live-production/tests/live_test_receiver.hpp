#pragma once

#include <atomic>
#include <cstdint>
#include <chrono>
#include <condition_variable>
#include <mutex>
#include <thread>
#include <vector>

#include <srt/srt.h>

class LiveTestReceiver {
 public:
  LiveTestReceiver();
  ~LiveTestReceiver();
  bool start(bool reject_first = false);
  uint32_t rejected() const { return rejected_.load(); }
  void stop();
  uint16_t port() const { return port_; }
  uint64_t bytes() const { return bytes_.load(); }
  uint64_t packets() const { return packets_.load(); }
  std::vector<uint8_t> data() const;
  std::vector<std::vector<uint8_t>> captures() const;
  bool wait_completed(size_t count, std::chrono::milliseconds timeout);

 private:
  void run();
  std::atomic<SRTSOCKET> listener_{SRT_INVALID_SOCK};
  std::atomic<SRTSOCKET> peer_{SRT_INVALID_SOCK};
  uint16_t port_ = 0;
  std::atomic<uint32_t> rejected_{0};
  std::atomic<bool> stopped_{false};
  std::atomic<uint64_t> bytes_{0};
  std::atomic<uint64_t> packets_{0};
  mutable std::mutex data_mutex_;
  std::vector<uint8_t> data_;
  std::vector<std::vector<uint8_t>> captures_;
  size_t completed_ = 0;
  std::condition_variable completed_changed_;
  std::thread worker_;
};
