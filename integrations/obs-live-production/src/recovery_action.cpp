#include "recovery_action.hpp"

namespace live_production {

bool RecoveryActionHandler::handle(ActionDecision decision,
                                   RecoveryModel &model,
                                   const RecoveryAction &action,
                                   const Persist &persist,
                                   const Enqueue &enqueue,
                                   std::string &error) {
  if (decision == ActionDecision::Execute)
    return accept(model, action, persist, enqueue, error);
  return decision == ActionDecision::DuplicateStarted ||
         decision == ActionDecision::DuplicateFailed;
}

bool RecoveryActionHandler::accept(RecoveryModel &model,
                                   const RecoveryAction &action,
                                   const Persist &persist,
                                   const Enqueue &enqueue,
                                   std::string &error) {
  if (pending_action_) {
    error = "recovery action is already in flight";
    return false;
  }
  model.persist_command(action.command_id);
  if (!persist(error)) {
    fail_closed(model, persist, error);
    return false;
  }
  pending_action_ = action;
  if (!enqueue(action.intent_epoch)) {
    error = "cannot queue publisher restart";
    fail_closed(model, persist, error);
    return false;
  }
  return true;
}

void RecoveryActionHandler::execute(RecoveryModel &model,
                                    uint64_t intent_epoch,
                                    const Persist &persist,
                                    const Restart &restart,
                                    std::string &error) {
  if (!pending_action_ || model.state().desired != LocalDesired::Running ||
      model.state().intent_epoch != intent_epoch ||
      model.state().last_command_id != pending_action_->command_id) {
    pending_action_.reset();
    return;
  }
  ++execution_count_;
  const bool started = restart(error);
  model.complete_command(started ? ActionOutcome::Started : ActionOutcome::Failed);
  if (!persist(error)) model.manual_stop();
  pending_action_.reset();
}

void RecoveryActionHandler::cancel() { pending_action_.reset(); }

bool RecoveryActionHandler::cancel_pending(RecoveryModel &model,
                                           const Persist &persist,
                                           std::string &error) {
  if (!pending_action_) return true;
  model.complete_command(ActionOutcome::Failed);
  const bool saved = persist(error);
  if (!saved) model.manual_stop();
  pending_action_.reset();
  return saved;
}

std::string RecoveryActionHandler::pending_server_run_id() const {
  return pending_action_ ? pending_action_->server_run_id : "";
}

void RecoveryActionHandler::fail_closed(RecoveryModel &model,
                                        const Persist &persist,
                                        std::string &error) {
  model.complete_command(ActionOutcome::Failed);
  if (!persist(error)) model.manual_stop();
  pending_action_.reset();
}

}
