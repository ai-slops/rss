#include "recovery_action.hpp"

#include <iostream>
#include <stdexcept>

namespace {

void require(bool value, const char *message) {
  if (!value) throw std::runtime_error(message);
}

live_production::RecoveryAction action(const live_production::RecoveryModel &model) {
  return {"restartPublisher", "command", "run", "controller",
          model.state().process_epoch, model.state().intent_epoch, 1};
}

void completed_duplicate_does_not_execute_handler_twice() {
  live_production::RecoveryModel model({});
  require(model.begin_process() && model.manual_start(), "model start failed");
  live_production::RecoveryActionHandler handler;
  uint64_t queued_epoch = 0;
  std::string error;
  const auto persist = [](std::string &) { return true; };
  require(handler.accept(model, action(model), persist,
                         [&](uint64_t epoch) { queued_epoch = epoch; return true; }, error),
          "action was not accepted");
  handler.execute(model, queued_epoch, persist,
                  [](std::string &) { return true; }, error);
  require(handler.execution_count() == 1, "first action did not execute once");
  require(model.state().last_command_result == live_production::ActionOutcome::Started,
          "started acknowledgement was not persisted");
  require(!handler.pending(), "completed action remained pending");
  require(handler.handle(live_production::ActionDecision::DuplicateStarted,
                         model, action(model), persist,
                         [](uint64_t) { return false; }, error),
          "completed duplicate acknowledgement was rejected");
  require(handler.execution_count() == 1,
          "completed duplicate changed restart execution count");
}

void enqueue_failure_records_failure_and_disarms_when_persist_fails() {
  live_production::RecoveryModel model({});
  require(model.begin_process() && model.manual_start(), "model start failed");
  live_production::RecoveryActionHandler handler;
  size_t persist_calls = 0;
  std::string error;
  const bool accepted = handler.accept(
      model, action(model),
      [&](std::string &) { return ++persist_calls == 1; },
      [](uint64_t) { return false; }, error);
  require(!accepted, "enqueue failure was accepted");
  require(model.state().desired == live_production::LocalDesired::Stopped,
          "failed action did not disarm after acknowledgement persistence failed");
  require(!handler.pending(), "failed action remained pending");
}

void manual_stop_cancels_held_persisted_action_before_restart() {
  live_production::RecoveryModel model({});
  require(model.begin_process() && model.manual_start(), "model start failed");
  live_production::RecoveryActionHandler handler;
  uint64_t held_epoch = 0;
  size_t restarts = 0;
  std::string error;
  const auto persist = [](std::string &) { return true; };
  require(handler.accept(model, action(model), persist,
                         [&](uint64_t epoch) { held_epoch = epoch; return true; }, error),
          "held action was not accepted");
  require(handler.pending() && model.state().last_command_id == "command",
          "accepted action was not durably pending");

  require(handler.cancel_pending(model, persist, error),
          "held action cancellation was not persisted");
  require(model.state().last_command_result == live_production::ActionOutcome::Failed,
          "held action cancellation did not persist failed outcome");
  model.manual_stop();
  handler.execute(model, held_epoch, persist,
                  [&](std::string &) { ++restarts; return true; }, error);

  require(restarts == 0 && handler.execution_count() == 0,
          "cancelled held action restarted publisher");
  require(model.state().desired == live_production::LocalDesired::Stopped &&
              model.state().last_command_id.empty(),
          "manual stop did not durably fence held action");
}

void eligibility_and_controller_cancellation_never_resume_held_restart() {
  for (const char *boundary : {"policyDisabled", "maintenance", "sourceDisabled",
                               "serverRunChanged", "controllerError"}) {
    live_production::RecoveryModel model({});
    require(model.begin_process() && model.manual_start(), "model start failed");
    live_production::RecoveryActionHandler handler;
    uint64_t held_epoch = 0;
    size_t restarts = 0;
    std::string error;
    const auto persist = [](std::string &) { return true; };
    require(handler.accept(model, action(model), persist,
                           [&](uint64_t epoch) { held_epoch = epoch; return true; }, error),
            "boundary action was not accepted");
    require(handler.cancel_pending(model, persist, error), boundary);
    handler.execute(model, held_epoch, persist,
                    [&](std::string &) { ++restarts; return true; }, error);
    require(restarts == 0 && handler.execution_count() == 0,
            "cancelled boundary action restarted publisher");
    require(model.state().desired == live_production::LocalDesired::Running,
            "eligibility cancellation stopped manual publisher intent");
  }
}

}

int main() {
  try {
    completed_duplicate_does_not_execute_handler_twice();
    enqueue_failure_records_failure_and_disarms_when_persist_fails();
    manual_stop_cancels_held_persisted_action_before_restart();
    eligibility_and_controller_cancellation_never_resume_held_restart();
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
