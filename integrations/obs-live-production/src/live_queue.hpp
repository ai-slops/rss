#pragma once

#include "live_packet.hpp"

#include <chrono>
#include <condition_variable>
#include <cstdint>
#include <deque>
#include <mutex>
#include <optional>

namespace live_production {

struct LiveWork {
  LivePacket packet;
  int epoch = 0;
};

class LiveQueue {
 public:
  enum class PushResult { Accepted, Dropped, Failed, Stopped };
  static constexpr size_t max_packets = 64;
  static constexpr size_t max_bytes = 4 * 1024 * 1024;
  explicit LiveQueue(int initial_epoch = 0) : epoch_(initial_epoch) {}

  PushResult push(encoder_packet *packet);
  std::optional<LiveWork> pop();
  bool begin_processing(int epoch);
  void end_processing();
  bool begin_send(int epoch);
  bool discard_send() const;
  void end_send();
  bool freeze(int &epoch, std::chrono::milliseconds timeout);
  bool resume(int epoch);
  void stop();
  void fail();
  bool failed() const;
  size_t packet_count() const;
  size_t byte_count() const;
  int epoch() const;

 private:
  enum class State { Running, Freezing, Frozen, Stopped };
  void clear_locked();
  mutable std::mutex mutex_;
  std::condition_variable changed_;
  std::deque<LiveWork> packets_;
  size_t bytes_ = 0;
  int epoch_ = 0;
  int acknowledged_epoch_ = 0;
  State state_ = State::Running;
  bool need_keyframe_ = false;
  bool sending_ = false;
  bool processing_ = false;
  int processing_epoch_ = 0;
  bool failed_ = false;
};

}
