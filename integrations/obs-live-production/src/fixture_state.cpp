#include "fixture_state.hpp"

namespace live_production {

std::optional<uint64_t> FixtureEvaluationState::request(bool active) {
  std::lock_guard<std::mutex> lock(mutex_);
  if (!active)
    return std::nullopt;
  pending_ = true;
  return queue_pending();
}

bool FixtureEvaluationState::begin_task(uint64_t generation, bool active) {
  std::lock_guard<std::mutex> lock(mutex_);
  if (!active || generation != generation_)
    return false;
  queued_ = false;
  if (transition_depth_ != 0 || evaluating_) {
    pending_ = true;
    return false;
  }
  evaluating_ = true;
  return true;
}

std::optional<uint64_t> FixtureEvaluationState::finish_task(bool active) {
  std::lock_guard<std::mutex> lock(mutex_);
  evaluating_ = false;
  return active ? queue_pending() : std::nullopt;
}

void FixtureEvaluationState::cancel_task(uint64_t generation) {
  std::lock_guard<std::mutex> lock(mutex_);
  if (generation == generation_) {
    queued_ = false;
    pending_ = true;
  }
}

void FixtureEvaluationState::begin_transition() {
  std::lock_guard<std::mutex> lock(mutex_);
  ++transition_depth_;
}

std::optional<uint64_t> FixtureEvaluationState::end_transition(bool active) {
  std::lock_guard<std::mutex> lock(mutex_);
  if (transition_depth_ != 0)
    --transition_depth_;
  return active ? queue_pending() : std::nullopt;
}

void FixtureEvaluationState::invalidate() {
  std::lock_guard<std::mutex> lock(mutex_);
  ++generation_;
  queued_ = false;
  pending_ = false;
}

std::optional<uint64_t> FixtureEvaluationState::queue_pending() {
  if (!pending_ || queued_ || evaluating_ || transition_depth_ != 0)
    return std::nullopt;
  pending_ = false;
  queued_ = true;
  return generation_;
}

}
