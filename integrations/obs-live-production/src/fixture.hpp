#pragma once

#include "config.hpp"
#include "publisher.hpp"

#include <obs.h>

#include <atomic>
#include <cstdint>
#include <memory>
#include <string>
#include <vector>

namespace live_production {

class FixtureController {
public:
  ~FixtureController();
  bool start_from_environment(std::string &error);
  void stop();

private:
  struct PrimarySender {
    FixtureSourceCommand declaration;
    std::unique_ptr<Publisher> publisher;
  };

  struct NegativeAttempt {
    std::string kind;
    PublisherConfig config;
    std::unique_ptr<Publisher> publisher;
    std::string result = "pending";
    bool stopped = false;
  };

  static void evaluate_task(void *context);
  static void deadline_tick(void *context, float seconds);
  void schedule_evaluate();
  void evaluate();
  bool prepare(std::string &error);
  bool start_negative_attempts();
  void write_final_report(const std::string &state);

  FixtureConfig fixture_;
  ControlConfig control_;
  std::vector<obs_source_t *> fixture_sources_;
  std::vector<PrimarySender> primaries_;
  std::vector<NegativeAttempt> negatives_;
  std::atomic<bool> active_ = false;
  std::atomic<bool> evaluation_queued_ = false;
  std::atomic<bool> timed_out_ = false;
  std::atomic<uint64_t> last_tick_queue_ns_ = 0;
  uint64_t deadline_ns_ = 0;
  bool negatives_started_ = false;
  bool report_written_ = false;
  bool tick_registered_ = false;
};

}
