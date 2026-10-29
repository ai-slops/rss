#include "recovery_model.hpp"

#include <iostream>
#include <stdexcept>

namespace {

void require(bool value, const char *message) {
  if (!value)
    throw std::runtime_error(message);
}

live_production::RecoveryFence fence(const live_production::RecoveryModel &model) {
  return {"source", "controller", "run", model.state().process_epoch,
          model.state().poll_seq, model.state().intent_epoch, true, true, false};
}

live_production::RecoveryAction action(const live_production::RecoveryModel &model,
                                       const char *id) {
  return {"restartPublisher", id, "run", "controller", model.state().process_epoch,
          model.state().intent_epoch, 1};
}

void new_process_is_stopped_and_old_reply_is_fenced() {
  // Given
  live_production::RecoveryModel model(
      {4, 9, 8, live_production::LocalDesired::Running, "", std::nullopt});
  require(model.begin_process(), "process epoch did not advance");
  auto old = action(model, "old");
  old.process_epoch = 4;

  // When
  const auto decision = model.decide(fence(model), old, "source", "controller");

  // Then
  require(model.state().desired == live_production::LocalDesired::Stopped,
          "new process resumed running intent");
  require(decision == live_production::ActionDecision::Reject,
          "old process reply was accepted");
}

void manual_stop_fences_pending_action() {
  live_production::RecoveryModel model({});
  require(model.begin_process(), "process epoch did not advance");
  require(model.manual_start(), "manual start did not advance intent");
  const auto queued = action(model, "command");
  require(model.manual_stop(), "manual stop did not advance intent");
  require(model.decide(fence(model), queued, "source", "controller") ==
              live_production::ActionDecision::Reject,
          "manual stop did not fence queued action");
}

void duplicate_command_returns_same_ack_without_execute() {
  live_production::RecoveryModel model({});
  require(model.begin_process(), "process epoch did not advance");
  require(model.manual_start(), "manual start did not advance intent");
  const auto restart = action(model, "command");
  require(model.decide(fence(model), restart, "source", "controller") ==
              live_production::ActionDecision::Execute,
          "eligible action was rejected");
  model.persist_command("command");
  model.complete_command(live_production::ActionOutcome::Started);
  require(model.decide(fence(model), restart, "source", "controller") ==
              live_production::ActionDecision::DuplicateStarted,
          "duplicate action did not return persisted acknowledgement");
}

void disabled_or_maintenance_policy_rejects_action() {
  live_production::RecoveryModel model({});
  require(model.begin_process(), "process epoch did not advance");
  require(model.manual_start(), "manual start did not advance intent");
  const auto restart = action(model, "command");
  auto disabled = fence(model);
  disabled.policy_enabled = false;
  auto maintenance = fence(model);
  maintenance.maintenance = true;
  require(model.decide(disabled, restart, "source", "controller") ==
              live_production::ActionDecision::Reject,
          "disabled policy accepted action");
  require(model.decide(maintenance, restart, "source", "controller") ==
              live_production::ActionDecision::Reject,
          "maintenance policy accepted action");
}

void pending_duplicate_and_old_poll_are_rejected() {
  live_production::RecoveryModel model({});
  require(model.begin_process(), "process epoch did not advance");
  require(model.manual_start(), "manual start did not advance intent");
  require(model.next_poll().has_value(), "poll sequence did not advance");
  const auto restart = action(model, "command");
  model.persist_command("command");
  require(model.decide(fence(model), restart, "source", "controller") ==
              live_production::ActionDecision::Reject,
          "pending duplicate executed twice");
  require(model.decide(fence(model), action(model, "other"), "source", "controller") ==
              live_production::ActionDecision::Reject,
          "second command replaced an in-flight action");
  auto old_poll = fence(model);
  --old_poll.poll_seq;
  const auto other = action(model, "other");
  require(model.decide(old_poll, other, "source", "controller") ==
              live_production::ActionDecision::Reject,
          "old poll response was accepted");
}

void epoch_overflow_rejects_start_but_stop_disarms() {
  live_production::RecoveryModel model(
      {1, UINT64_MAX, UINT64_MAX, live_production::LocalDesired::Running, "", std::nullopt});
  require(!model.manual_start(), "overflowing manual start was accepted");
  require(!model.manual_stop(), "overflowing stop reported an epoch advance");
  require(model.state().desired == live_production::LocalDesired::Stopped,
          "overflowing stop did not disarm recovery");
  require(!model.next_poll().has_value(), "overflowing poll sequence was reused");
}

void explicit_start_waits_for_stopped_registration() {
  live_production::RecoveryStartGate gate;
  require(!gate.request_start(), "start executed before registration");
  require(gate.pending() && !gate.ready(), "start gesture was not queued");
  require(gate.register_stopped(), "queued start was lost after registration");
  require(gate.ready() && !gate.pending(), "registration gate did not settle");
  gate.cancel();
  require(!gate.ready() && !gate.pending(), "manual stop did not clear registration gate");
}

}

int main() {
  try {
    new_process_is_stopped_and_old_reply_is_fenced();
    manual_stop_fences_pending_action();
    duplicate_command_returns_same_ack_without_execute();
    disabled_or_maintenance_policy_rejects_action();
    pending_duplicate_and_old_poll_are_rejected();
    epoch_overflow_rejects_start_but_stop_disarms();
    explicit_start_waits_for_stopped_registration();
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
