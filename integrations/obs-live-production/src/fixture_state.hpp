#pragma once

#include <cstdint>
#include <mutex>
#include <optional>

namespace live_production {

class FixtureEvaluationState {
public:
  std::optional<uint64_t> request(bool active);
  bool begin_task(uint64_t generation, bool active);
  std::optional<uint64_t> finish_task(bool active);
  void cancel_task(uint64_t generation);
  void begin_transition();
  std::optional<uint64_t> end_transition(bool active);
  void invalidate();

private:
  std::optional<uint64_t> queue_pending();

  std::mutex mutex_;
  uint64_t generation_ = 1;
  uint32_t transition_depth_ = 0;
  bool evaluating_ = false;
  bool queued_ = false;
  bool pending_ = false;
};

}
