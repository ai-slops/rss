#include "module_runtime.hpp"

#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonValue>
#include <QString>

namespace live_production {

std::optional<PollDispatch> ModuleRuntime::build_poll() {
  if (!model_ || !publisher_.config()) return std::nullopt;
  const auto sequence = model_->next_poll();
  if (!sequence) {
    runtime_error_ = "recovery poll sequence is exhausted";
    model_->manual_stop();
    publisher_.stop();
    client_.stop();
    return std::nullopt;
  }
  std::string error;
  if (!persist(error)) {
    runtime_error_ = error;
    return std::nullopt;
  }
  QJsonObject root{{"schemaVersion", 1},
                   {"controllerId", QString::fromStdString(
                                        publisher_.config()->recovery->controller_id)},
                    {"processEpoch", QString::number(static_cast<qulonglong>(model_->state().process_epoch))},
                    {"pollSeq", QString::number(static_cast<qulonglong>(*sequence))},
                    {"intentEpoch", QString::number(static_cast<qulonglong>(model_->state().intent_epoch))},
                   {"localDesired", QString::fromLatin1(
                                        desired_name(model_->state().desired))},
                   {"publisherState", QString::fromStdString(publisher_.state())}};
  if (!model_->state().last_command_id.empty() &&
      model_->state().last_command_result) {
    root.insert("lastAction",
                QJsonObject{{"commandId", QString::fromStdString(
                                              model_->state().last_command_id)},
                            {"result", QString::fromLatin1(outcome_name(
                                           *model_->state().last_command_result))}});
  } else {
    root.insert("lastAction", QJsonValue::Null);
  }
  return PollDispatch{QJsonDocument(root).toJson(QJsonDocument::Compact),
                      {model_->state().process_epoch, *sequence,
                       model_->state().intent_epoch}};
}

void ModuleRuntime::handle_response(const PollContext &request,
                                    const RecoveryPollResponse &response) {
  if (!model_ || !publisher_.config() ||
      request.process_epoch != model_->state().process_epoch ||
      request.poll_seq != model_->state().poll_seq ||
      request.intent_epoch != model_->state().intent_epoch ||
      response.echoed.process_epoch != request.process_epoch ||
      response.echoed.poll_seq != request.poll_seq ||
      response.echoed.intent_epoch != request.intent_epoch)
    return;
  if (response.fence.source_id != publisher_.config()->target.source_id ||
      response.fence.controller_id != publisher_.config()->recovery->controller_id) {
    handle_client_error("recovery response source/controller scope mismatch");
    return;
  }
  recovery_online_ = response.controller_online;
  receiver_state_ = response.receiver_state;
  retry_result_ = response.retry_result;
  retry_attempts_ = response.retry_attempts;
  const bool run_queued_start = model_->state().desired == LocalDesired::Stopped &&
                                response.controller_local_desired == "stopped" &&
                                start_gate_.register_stopped();
  const bool eligible = response.fence.source_enabled &&
                        response.fence.policy_enabled &&
                        !response.fence.maintenance && response.controller_online;
  const bool same_server = !action_handler_.pending() ||
                           action_handler_.pending_server_run_id() ==
                               response.fence.server_run_id;
  if (action_handler_.pending() && (!eligible || !same_server)) {
    cancel_pending_action(!same_server ? "server run changed while recovery action was pending"
                                      : "recovery eligibility was withdrawn");
    return;
  }
  if (response.action) {
    last_action_response_ = response;
  }
  if (run_queued_start) {
    std::string error;
    perform_manual_start(error);
  }
  if (!response.action) return;
  const RecoveryConfig &recovery = *publisher_.config()->recovery;
  const ActionDecision decision = model_->decide(
      response.fence, *response.action, publisher_.config()->target.source_id,
      recovery.controller_id);
  std::string error;
  if (decision == ActionDecision::Execute) {
    execute_action(*response.action);
  } else if (decision == ActionDecision::DuplicateStarted ||
             decision == ActionDecision::DuplicateFailed) {
    action_handler_.handle(
        decision, *model_, *response.action,
        [this](std::string &failure) { return persist(failure); },
        [](uint64_t) { return false; }, error);
  }
}

void ModuleRuntime::handle_client_error(const std::string &error) {
  cancel_pending_action("controller error cancelled pending recovery action");
  recovery_online_ = false;
  runtime_error_ = error;
}

}
