#include "fixture.hpp"

#include "report.hpp"

#include <obs-module.h>
#include <util/platform.h>

#include <cstdlib>

namespace live_production {
namespace {

std::string report_kind(const std::string &command_kind) {
  return command_kind == "startBadAuthSender" ? "badAuth"
                                                : "duplicateSender";
}

}

FixtureController::~FixtureController() { stop(); }

bool FixtureController::start_from_environment(std::string &error) {
  const char *fixture_path = std::getenv("LIVE_PRODUCTION_OBS_FIXTURE_FILE");
  if (!fixture_path || !*fixture_path)
    return true;
  if (!load_fixture_config(fixture_path, fixture_, error))
    return false;
  if (!load_control_config(fixture_.control_file, control_, error) ||
      !prepare(error)) {
    write_final_report("failed");
    return false;
  }

  active_.store(true);
  deadline_ns_ = os_gettime_ns() +
                 static_cast<uint64_t>(fixture_.deadline_seconds) *
                     1000000000ULL;
  obs_add_tick_callback(deadline_tick, this);
  tick_registered_ = true;
  for (PrimarySender &sender : primaries_) {
    if (!sender.publisher->initialize(error) ||
        !sender.publisher->start(error)) {
      write_final_report("failed");
      return false;
    }
  }
  schedule_evaluate();
  return true;
}

void FixtureController::schedule_evaluate() {
  if (!active_.load() || evaluation_queued_.exchange(true))
    return;
  obs_queue_task(OBS_TASK_UI, evaluate_task, this, false);
}

void FixtureController::evaluate_task(void *context) {
  auto *controller = static_cast<FixtureController *>(context);
  controller->evaluation_queued_.store(false);
  controller->evaluate();
}

void FixtureController::deadline_tick(void *context, float) {
  auto *controller = static_cast<FixtureController *>(context);
  if (!controller->active_.load())
    return;
  const uint64_t now = os_gettime_ns();
  if (now >= controller->deadline_ns_)
    controller->timed_out_.store(true);
  uint64_t previous = controller->last_tick_queue_ns_.load();
  if (now >= previous + 250000000ULL &&
      controller->last_tick_queue_ns_.compare_exchange_strong(previous, now))
    controller->schedule_evaluate();
}

void FixtureController::evaluate() {
  if (!active_.load() || report_written_)
    return;
  bool all_sending = !primaries_.empty();
  for (const PrimarySender &sender : primaries_) {
    const std::string state = sender.publisher->state();
    if (state == "failed" || state == "reconnecting" || state == "stopped") {
      write_final_report("failed");
      return;
    }
    all_sending = all_sending && state == "sending";
  }
  if (all_sending && !negatives_started_ && !start_negative_attempts()) {
    write_final_report("failed");
    return;
  }
  if (negatives_started_) {
    bool all_rejected = true;
    for (NegativeAttempt &attempt : negatives_) {
      const std::string state = attempt.publisher->state();
      if (attempt.result == "pending" &&
          (state == "failed" || state == "reconnecting"))
        attempt.result = "rejected";
      if (attempt.result == "rejected" && !attempt.stopped) {
        attempt.publisher->stop();
        attempt.stopped = true;
      }
      all_rejected = all_rejected && attempt.result == "rejected";
    }
    if (all_rejected && all_sending) {
      write_final_report("ready");
      return;
    }
  }
  if (timed_out_.load()) {
    for (NegativeAttempt &attempt : negatives_) {
      if (attempt.result == "pending")
        attempt.result = attempt.publisher &&
                                 attempt.publisher->state() == "sending"
                             ? "accepted"
                             : "notObserved";
    }
    write_final_report("deadlineExceeded");
  }
}

void FixtureController::write_final_report(const std::string &state) {
  std::vector<SenderEvidence> senders;
  for (const PrimarySender &sender : primaries_) {
    senders.push_back({sender.publisher->config().target.source_id,
                       sender.declaration.source_name,
                       sender.declaration.audio_mix_index,
                       sender.publisher->state(),
                       sender.publisher->last_error(),
                       sender.declaration.color,
                       sender.declaration.frame_counter,
                       sender.declaration.tone_hz});
  }
  std::vector<NegativeEvidence> negative_cases;
  for (const NegativeAttempt &attempt : negatives_)
    negative_cases.push_back({report_kind(attempt.kind), attempt.result});
  std::string error;
  if (write_report(fixture_.report_file, state, senders, negative_cases, error))
    report_written_ = true;
  else
    blog(LOG_ERROR, "[obs-live-production] %s", error.c_str());
}

void FixtureController::stop() {
  if (!active_.exchange(false) && fixture_sources_.empty())
    return;
  if (tick_registered_) {
    obs_remove_tick_callback(deadline_tick, this);
    tick_registered_ = false;
  }
  for (NegativeAttempt &attempt : negatives_) {
    if (attempt.publisher)
      attempt.publisher->stop();
  }
  for (PrimarySender &sender : primaries_)
    sender.publisher->stop();
  negatives_.clear();
  primaries_.clear();
  for (obs_source_t *source : fixture_sources_)
    obs_source_release(source);
  fixture_sources_.clear();
}

}
