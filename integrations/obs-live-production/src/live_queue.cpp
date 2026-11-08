#include "live_queue.hpp"

namespace live_production {

LiveQueue::PushResult LiveQueue::push(encoder_packet *packet) {
  if (!packet) return PushResult::Failed;
  std::lock_guard<std::mutex> lock(mutex_);
  if (state_ == State::Stopped) return PushResult::Stopped;
  if (state_ != State::Running) return PushResult::Dropped;
  if (need_keyframe_) {
    if (packet->type != OBS_ENCODER_VIDEO || !packet->keyframe)
      return PushResult::Dropped;
    need_keyframe_ = false;
  }
  if (packets_.size() >= max_packets || packet->size > max_bytes - bytes_) {
    failed_ = true;
    state_ = State::Stopped;
    clear_locked();
    changed_.notify_all();
    return PushResult::Failed;
  }
  packets_.push_back(LiveWork{LivePacket(packet), epoch_});
  bytes_ += packet->size;
  changed_.notify_one();
  return PushResult::Accepted;
}

std::optional<LiveWork> LiveQueue::pop() {
  std::unique_lock<std::mutex> lock(mutex_);
  changed_.wait(lock, [this] { return state_ == State::Stopped || !packets_.empty(); });
  if (packets_.empty()) return std::nullopt;
  LiveWork work = std::move(packets_.front());
  packets_.pop_front();
  bytes_ -= work.packet.size();
  return work;
}

bool LiveQueue::begin_processing(int epoch) {
  std::lock_guard<std::mutex> lock(mutex_);
  if (state_ != State::Running || epoch != epoch_) return false;
  processing_ = true;
  processing_epoch_ = epoch;
  return true;
}

void LiveQueue::end_processing() {
  std::lock_guard<std::mutex> lock(mutex_);
  processing_ = false;
  changed_.notify_all();
}

bool LiveQueue::begin_send(int epoch) {
  std::lock_guard<std::mutex> lock(mutex_);
  if (!processing_ || epoch != processing_epoch_ || state_ == State::Stopped ||
      state_ == State::Frozen || (state_ == State::Running && epoch != epoch_))
    return false;
  sending_ = true;
  return true;
}

bool LiveQueue::discard_send() const {
  std::lock_guard<std::mutex> lock(mutex_);
  return state_ == State::Freezing || state_ == State::Frozen;
}

void LiveQueue::end_send() {
  std::lock_guard<std::mutex> lock(mutex_);
  sending_ = false;
  changed_.notify_all();
}

bool LiveQueue::freeze(int &epoch, std::chrono::milliseconds timeout) {
  std::unique_lock<std::mutex> lock(mutex_);
  const auto deadline = std::chrono::steady_clock::now() + timeout;
  while (state_ == State::Freezing) {
    if (!changed_.wait_until(lock, deadline,
                             [this] { return state_ != State::Freezing; }))
      return false;
  }
  if (state_ == State::Stopped) return false;
  if (state_ == State::Frozen) {
    epoch = epoch_;
    return true;
  }
  state_ = State::Freezing;
  ++epoch_;
  clear_locked();
  if (!changed_.wait_until(lock, deadline,
                           [this] {
                             return (!processing_ && !sending_) ||
                                    state_ == State::Stopped;
                           })) {
    if (state_ == State::Freezing) state_ = State::Running;
    need_keyframe_ = true;
    changed_.notify_all();
    return false;
  }
  if (state_ != State::Freezing) return false;
  epoch = epoch_;
  acknowledged_epoch_ = epoch_;
  state_ = State::Frozen;
  changed_.notify_all();
  return true;
}

bool LiveQueue::resume(int epoch) {
  std::lock_guard<std::mutex> lock(mutex_);
  if (state_ == State::Stopped || acknowledged_epoch_ == 0 || epoch != epoch_ ||
      epoch != acknowledged_epoch_) return false;
  if (state_ == State::Running) return true;
  if (state_ != State::Frozen) return false;
  state_ = State::Running;
  need_keyframe_ = true;
  changed_.notify_all();
  return true;
}

void LiveQueue::stop() {
  std::lock_guard<std::mutex> lock(mutex_);
  state_ = State::Stopped;
  clear_locked();
  changed_.notify_all();
}

void LiveQueue::fail() {
  std::lock_guard<std::mutex> lock(mutex_);
  failed_ = true;
  state_ = State::Stopped;
  clear_locked();
  changed_.notify_all();
}

bool LiveQueue::failed() const {
  std::lock_guard<std::mutex> lock(mutex_);
  return failed_;
}

size_t LiveQueue::packet_count() const {
  std::lock_guard<std::mutex> lock(mutex_);
  return packets_.size();
}

size_t LiveQueue::byte_count() const {
  std::lock_guard<std::mutex> lock(mutex_);
  return bytes_;
}

int LiveQueue::epoch() const {
  std::lock_guard<std::mutex> lock(mutex_);
  return epoch_;
}

void LiveQueue::clear_locked() {
  packets_.clear();
  bytes_ = 0;
}

}
