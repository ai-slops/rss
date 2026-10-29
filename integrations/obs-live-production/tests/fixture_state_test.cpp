#include "fixture_state.hpp"

#include <atomic>
#include <iostream>
#include <memory>
#include <optional>
#include <stdexcept>
#include <string>
#include <thread>
#include <vector>

namespace {

struct Attempt {
  std::string state = "created";
  std::string result = "pending";
};

void require(bool condition, const char *message) {
  if (!condition)
    throw std::runtime_error(message);
}

class Harness {
public:
  void construct_from_evaluation(bool first_fails, bool preconstruct) {
    const auto generation = state_.request(active_);
    if (!generation || !state_.begin_task(*generation, active_))
      return;
    construct_negatives(first_fails, preconstruct);
    dispatch(state_.finish_task(active_));
  }

  void construct_negatives(bool first_fails, bool preconstruct) {
    state_.begin_transition();
    attempts_.resize(2);
    if (preconstruct) {
      for (auto &attempt : attempts_)
        attempt = std::make_unique<Attempt>();
    }
    for (size_t index = 0; index < attempts_.size(); ++index) {
      if (!attempts_[index])
        attempts_[index] = std::make_unique<Attempt>();
      if (index == 0 && first_fails)
        attempts_[index]->state = "failed";
      request();
      if (index == 0 && first_fails)
        attempts_[index]->result = "fixtureError";
    }
    dispatch(state_.end_transition(active_));
  }

  void request() { dispatch(state_.request(active_)); }

  void stop_during_transition() {
    state_.begin_transition();
    active_ = false;
    state_.invalidate();
    request();
    dispatch(state_.end_transition(active_));
  }

  bool partial_observed() const { return partial_observed_; }
  const std::string &first_result() const { return attempts_[0]->result; }

private:
  void dispatch(std::optional<uint64_t> generation) {
    if (!generation || !state_.begin_task(*generation, active_))
      return;
    evaluate();
    dispatch(state_.finish_task(active_));
  }

  void evaluate() {
    for (const auto &attempt : attempts_) {
      if (!attempt) {
        partial_observed_ = true;
        continue;
      }
      if (attempt->result == "pending" && attempt->state == "failed")
        attempt->result = "rejected";
    }
  }

  live_production::FixtureEvaluationState state_;
  std::vector<std::unique_ptr<Attempt>> attempts_;
  bool active_ = true;
  bool partial_observed_ = false;
};

void synchronous_callback_does_not_observe_partial_construction() {
  // Given
  Harness harness;

  // When
  harness.construct_from_evaluation(false, false);

  // Then
  require(!harness.partial_observed(),
          "synchronous callback observed partial construction");
}

void initialization_failure_is_not_admission_rejection() {
  // Given
  Harness harness;

  // When
  harness.construct_from_evaluation(true, true);

  // Then
  require(harness.first_result() == "fixtureError",
          "initialization failure was classified as admission rejection");
}

void stale_task_cannot_consume_new_generation() {
  // Given
  live_production::FixtureEvaluationState state;
  const auto stale = state.request(true);
  state.invalidate();
  const auto current = state.request(true);

  // When
  const bool stale_started = stale && state.begin_task(*stale, true);
  const bool current_started = current && state.begin_task(*current, true);

  // Then
  require(!stale_started, "stale evaluation task started");
  require(current_started, "current evaluation task was consumed by stale task");
}

void cleanup_callbacks_do_not_schedule_after_invalidation() {
  // Given
  Harness harness;

  // When
  harness.stop_during_transition();

  // Then
  require(!harness.partial_observed(),
          "cleanup callback evaluated invalidated fixture state");
}

void nested_transitions_release_only_after_outer_completion() {
  // Given
  live_production::FixtureEvaluationState state;
  state.begin_transition();
  state.begin_transition();
  const auto during = state.request(true);

  // When
  const auto inner = state.end_transition(true);
  const auto outer = state.end_transition(true);

  // Then
  require(!during, "transition queued evaluation immediately");
  require(!inner, "inner transition released pending evaluation");
  require(outer.has_value(), "outer transition did not release evaluation");
}

void allocation_failure_can_retry_the_same_generation() {
  // Given
  live_production::FixtureEvaluationState state;
  const auto first = state.request(true);
  require(first.has_value(), "initial evaluation was not queued");

  // When
  state.cancel_task(*first);
  const auto retry = state.request(true);

  // Then
  require(retry == first, "cancelled evaluation did not retry its generation");
}

void concurrent_callbacks_coalesce_during_transition() {
  // Given
  live_production::FixtureEvaluationState state;
  state.begin_transition();
  std::atomic<size_t> immediate = 0;
  std::vector<std::thread> callbacks;
  for (size_t index = 0; index < 16; ++index) {
    callbacks.emplace_back([&] {
      if (state.request(true))
        immediate.fetch_add(1);
    });
  }

  // When
  for (std::thread &callback : callbacks)
    callback.join();
  const auto queued = state.end_transition(true);

  // Then
  require(immediate.load() == 0, "callback queued during transition");
  require(queued.has_value(), "coalesced callback was not queued");
}

}

int main() {
  try {
    synchronous_callback_does_not_observe_partial_construction();
    initialization_failure_is_not_admission_rejection();
    stale_task_cannot_consume_new_generation();
    cleanup_callbacks_do_not_schedule_after_invalidation();
    nested_transitions_release_only_after_outer_completion();
    allocation_failure_can_retry_the_same_generation();
    concurrent_callbacks_coalesce_during_transition();
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
