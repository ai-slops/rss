#include "module_runtime.hpp"

namespace live_production {

bool ModuleRuntime::execute_action(const RecoveryAction &action) {
  if (!model_) return false;
  std::string error;
  if (!ensure_action_dispatcher(error)) {
    runtime_error_ = error;
    return false;
  }
  const bool accepted = action_handler_.handle(
      ActionDecision::Execute, *model_, action,
      [this](std::string &failure) { return persist(failure); },
      [this](uint64_t intent_epoch) {
        return action_dispatcher_.enqueue(intent_epoch);
      },
      error);
  if (model_->state().desired == LocalDesired::Stopped) publisher_.stop();
  if (!error.empty()) runtime_error_ = error;
  return accepted;
}

void ModuleRuntime::cancel_pending_action(const std::string &reason) {
  if (!model_ || !action_handler_.pending()) return;
  action_dispatcher_.close();
  action_dispatcher_open_ = false;
  std::string error;
  action_handler_.cancel_pending(
      *model_, [this](std::string &failure) { return persist(failure); }, error);
  if (model_->state().desired == LocalDesired::Stopped) publisher_.stop();
  runtime_error_ = error.empty() ? reason : error;
}

bool ModuleRuntime::ensure_action_dispatcher(std::string &error) {
  if (action_dispatcher_open_) return true;
  if (action_dispatcher_.open(
          [this](uint64_t intent_epoch) { run_action(intent_epoch); })) {
    action_dispatcher_open_ = true;
    return true;
  }
  error = "cannot create recovery action receiver";
  return false;
}

void ModuleRuntime::run_action(uint64_t intent_epoch) {
  if (!model_) return;
  std::string error;
  action_handler_.execute(
      *model_, intent_epoch,
      [this](std::string &failure) { return persist(failure); },
      [this](std::string &failure) { return publisher_.restart(failure); }, error);
  if (model_->state().desired == LocalDesired::Stopped) publisher_.stop();
  if (!error.empty()) runtime_error_ = error;
}

void ModuleRuntime::fault_drop_publisher() { publisher_.stop(); }
void ModuleRuntime::fault_pause_controller(bool paused) { client_.pause(paused); }
void ModuleRuntime::fault_force_restart_failure() {
  publisher_.force_restart_failures(3);
}

bool ModuleRuntime::fault_pause_action_dispatch(bool paused) {
  return action_dispatcher_open_ && action_dispatcher_.pause(paused);
}

FaultRedeliveryResult ModuleRuntime::fault_redeliver_last_action() {
  FaultRedeliveryResult result;
  if (!model_ || !last_action_response_ || !last_action_response_->action) {
    duplicate_command_id_.clear();
    duplicate_result_ = "notObserved";
    return result;
  }
  RecoveryPollResponse response = *last_action_response_;
  const PollContext context{model_->state().process_epoch, model_->state().poll_seq,
                            model_->state().intent_epoch};
  response.echoed = context;
  response.fence.process_epoch = context.process_epoch;
  response.fence.poll_seq = context.poll_seq;
  response.fence.intent_epoch = context.intent_epoch;
  const uint64_t before = action_handler_.execution_count();
  handle_response(context, response);
  result.command_id = response.action->command_id;
  result.observed = action_handler_.execution_count() == before &&
                    model_->state().last_command_id == result.command_id &&
                    model_->state().last_command_result.has_value();
  result.result = result.observed
                      ? outcome_name(*model_->state().last_command_result)
                      : "notObserved";
  duplicate_command_id_ = result.command_id;
  duplicate_result_ = result.result;
  return result;
}

}
