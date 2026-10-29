#include "recovery_model.hpp"

#include <utility>

namespace live_production {

RecoveryModel::RecoveryModel(PersistedRecoveryState state) : state_(std::move(state)) {}

bool RecoveryModel::begin_process() {
  if (state_.process_epoch == UINT64_MAX) return false;
  ++state_.process_epoch;
  state_.desired = LocalDesired::Stopped;
  state_.last_command_id.clear();
  state_.last_command_result.reset();
  return true;
}

bool RecoveryModel::manual_start() {
  if (state_.intent_epoch == UINT64_MAX) return false;
  ++state_.intent_epoch;
  state_.desired = LocalDesired::Running;
  return true;
}

bool RecoveryModel::manual_stop() {
  const bool advanced = state_.intent_epoch != UINT64_MAX;
  if (advanced)
    ++state_.intent_epoch;
  state_.desired = LocalDesired::Stopped;
  state_.last_command_id.clear();
  state_.last_command_result.reset();
  return advanced;
}

std::optional<uint64_t> RecoveryModel::next_poll() {
  if (state_.poll_seq == UINT64_MAX) return std::nullopt;
  ++state_.poll_seq;
  return state_.poll_seq;
}

ActionDecision RecoveryModel::decide(const RecoveryFence &fence,
                                     const RecoveryAction &action,
                                     const std::string &source_id,
                                     const std::string &controller_id) const {
  if (action.kind != "restartPublisher" || fence.source_id != source_id ||
      action.controller_id != controller_id || fence.controller_id != controller_id ||
      action.server_run_id != fence.server_run_id ||
      action.process_epoch != state_.process_epoch ||
      fence.process_epoch != state_.process_epoch ||
      fence.poll_seq != state_.poll_seq ||
      action.intent_epoch != state_.intent_epoch ||
      fence.intent_epoch != state_.intent_epoch ||
      state_.desired != LocalDesired::Running || !fence.source_enabled ||
      !fence.policy_enabled || fence.maintenance)
    return ActionDecision::Reject;
  if (action.command_id == state_.last_command_id && state_.last_command_result)
    return *state_.last_command_result == ActionOutcome::Started
               ? ActionDecision::DuplicateStarted
               : ActionDecision::DuplicateFailed;
  if (!state_.last_command_id.empty() && !state_.last_command_result)
    return ActionDecision::Reject;
  return ActionDecision::Execute;
}

void RecoveryModel::persist_command(const std::string &command_id) {
  state_.last_command_id = command_id;
  state_.last_command_result.reset();
}

void RecoveryModel::complete_command(ActionOutcome outcome) {
  state_.last_command_result = outcome;
}

bool RecoveryStartGate::request_start() {
  if (ready_) return true;
  pending_ = true;
  return false;
}

bool RecoveryStartGate::register_stopped() {
  ready_ = true;
  const bool start = pending_;
  pending_ = false;
  return start;
}

void RecoveryStartGate::cancel() {
  pending_ = false;
  ready_ = false;
}

const char *desired_name(LocalDesired value) {
  return value == LocalDesired::Running ? "running" : "stopped";
}

const char *outcome_name(ActionOutcome value) {
  return value == ActionOutcome::Started ? "started" : "failed";
}

}
