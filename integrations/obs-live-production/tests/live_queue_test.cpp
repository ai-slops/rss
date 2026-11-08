#include "live_queue.hpp"

#include <obs-module.h>

#include <chrono>
#include <atomic>
#include <condition_variable>
#include <iostream>
#include <mutex>
#include <stdexcept>
#include <thread>

namespace {

void require(bool value, const char *message) {
  if (!value) throw std::runtime_error(message);
}

encoder_packet packet(obs_encoder_type type, bool keyframe, size_t size) {
  encoder_packet value{};
  value.data = static_cast<uint8_t *>(bmalloc(size));
  value.size = size;
  value.type = type;
  value.keyframe = keyframe;
  value.timebase_num = 1;
  value.timebase_den = type == OBS_ENCODER_VIDEO ? 30 : 48000;
  return value;
}

void release(encoder_packet &value) { bfree(value.data); }

void freeze_resume_is_epoch_fenced() {
  live_production::LiveQueue first;
  live_production::LiveQueue second;
  live_production::LiveQueue replacement(5);
  require(!replacement.resume(5), "new session accepted a prior session resume");
  int epoch = 0;
  require(first.freeze(epoch, std::chrono::milliseconds(10)), "freeze failed");
  require(epoch == 1, "first epoch was not one");
  int repeated = 0;
  require(first.freeze(repeated, std::chrono::milliseconds(10)) && repeated == epoch,
          "repeat freeze changed epoch");
  require(second.begin_processing(0) && second.begin_send(0),
          "other publisher stopped sending");
  second.end_send();
  second.end_processing();
  require(!first.resume(epoch + 1), "stale resume was accepted");
  require(first.resume(epoch), "matching resume failed");
  require(first.resume(epoch), "idempotent resume failed");

  encoder_packet audio = packet(OBS_ENCODER_AUDIO, false, 32);
  encoder_packet delta = packet(OBS_ENCODER_VIDEO, false, 32);
  encoder_packet key = packet(OBS_ENCODER_VIDEO, true, 32);
  require(first.push(&audio) == live_production::LiveQueue::PushResult::Dropped &&
              first.push(&delta) == live_production::LiveQueue::PushResult::Dropped,
          "restart discard failed");
  require(first.packet_count() == 0, "pre-IDR packet escaped gate");
  require(first.push(&key) == live_production::LiveQueue::PushResult::Accepted &&
              first.packet_count() == 1,
          "IDR did not reopen gate");
  release(audio);
  release(delta);
  release(key);
}

void queue_limits_fail_closed() {
  live_production::LiveQueue queue;
  encoder_packet value = packet(OBS_ENCODER_VIDEO, true, 65536);
  for (size_t index = 0; index < live_production::LiveQueue::max_packets; ++index)
    require(queue.push(&value) == live_production::LiveQueue::PushResult::Accepted,
            "bounded packet was rejected");
  require(queue.push(&value) == live_production::LiveQueue::PushResult::Failed,
          "packet overflow was accepted");
  require(queue.failed(), "overflow did not fail queue");
  require(queue.packet_count() == 0 && queue.byte_count() == 0,
          "overflow retained stale packets");
  release(value);
}

void stop_cancels_pending_freezes() {
  live_production::LiveQueue queue;
  require(!queue.resume(0), "resume zero succeeded before freeze");
  require(queue.begin_processing(0) && queue.begin_send(0), "send barrier failed");
  std::mutex mutex;
  std::condition_variable changed;
  int entered = 0;
  bool first = true;
  bool second = true;
  const auto call = [&](bool &result) {
    {
      std::lock_guard<std::mutex> lock(mutex);
      ++entered;
      changed.notify_all();
    }
    int epoch = 0;
    result = queue.freeze(epoch, std::chrono::seconds(5));
  };
  std::thread one(call, std::ref(first));
  std::thread two(call, std::ref(second));
  {
    std::unique_lock<std::mutex> lock(mutex);
    changed.wait(lock, [&] { return entered == 2; });
  }
  queue.stop();
  queue.end_send();
  queue.end_processing();
  one.join();
  two.join();
  require(!first && !second, "freeze succeeded after Stop");
}

void popped_old_epoch_never_reopens_transport() {
  live_production::LiveQueue queue;
  encoder_packet value = packet(OBS_ENCODER_VIDEO, true, 32);
  require(queue.push(&value) == live_production::LiveQueue::PushResult::Accepted,
          "packet enqueue failed");
  auto old = queue.pop();
  require(old.has_value(), "packet pop failed");
  int epoch = 0;
  require(queue.freeze(epoch, std::chrono::milliseconds(10)), "freeze failed");
  require(queue.resume(epoch), "resume failed");
  require(!queue.begin_processing(old->epoch), "old packet processing resumed");
  require(!queue.begin_send(old->epoch), "old packet transport send resumed");
  release(value);
}

long long freeze_timeout_is_single_deadline() {
  live_production::LiveQueue queue;
  require(queue.begin_processing(0) && queue.begin_send(0), "timeout barrier failed");
  int epoch = 0;
  const auto started = std::chrono::steady_clock::now();
  require(!queue.freeze(epoch, std::chrono::milliseconds(100)),
          "blocked freeze unexpectedly succeeded");
  const auto elapsed = std::chrono::duration_cast<std::chrono::milliseconds>(
      std::chrono::steady_clock::now() - started);
  queue.end_send();
  queue.end_processing();
  require(elapsed < std::chrono::milliseconds(200), "freeze exceeded caller deadline");
  return elapsed.count();
}

void freezing_finishes_the_owned_packet_before_acknowledgement() {
  live_production::LiveQueue queue;
  require(queue.begin_processing(0), "processing barrier failed");
  std::atomic<bool> acknowledged{false};
  int frozen_epoch = 0;
  std::thread freezer([&] {
    acknowledged.store(queue.freeze(frozen_epoch, std::chrono::seconds(2)));
  });
  const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(1);
  while (queue.epoch() == 0 && std::chrono::steady_clock::now() < deadline)
    std::this_thread::yield();
  const bool draining = queue.begin_send(0);
  const bool premature = acknowledged.load();
  queue.end_send();
  queue.end_processing();
  freezer.join();
  require(draining, "freeze discarded the remainder of its in-flight mux packet");
  require(!premature && acknowledged.load(), "freeze acknowledged before packet drain");
  require(queue.resume(frozen_epoch), "drained transport could not resume");
}

}

int main() {
  try {
    freeze_resume_is_epoch_fenced();
    queue_limits_fail_closed();
    stop_cancels_pending_freezes();
    popped_old_epoch_never_reopens_transport();
    freezing_finishes_the_owned_packet_before_acknowledgement();
    const auto freeze_ms = freeze_timeout_is_single_deadline();
    std::cout << "live queue tests passed freeze_timeout_ms=" << freeze_ms << '\n';
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
