#include "recovery_fixture.hpp"
#include "module_runtime.hpp"
#include "recovery_fault.hpp"

#include <QTimer>
#include <algorithm>

namespace live_production {

void RecoveryFixture::execute_command(uint64_t command_id, const std::string &kind,
                                      uint32_t duration_ms) {
  std::vector<std::string> allowlist;
  allowlist.reserve(operations_.size());
  for (const RecoveryFaultOperation &operation : operations_)
    allowlist.push_back(operation.kind);
  if (!accept_recovery_fault(last_command_id_, command_id, kind, allowlist)) {
    last_command_result_ = "rejectedNotAllowlisted";
    write_report("running");
    return;
  }
  last_command_id_ = command_id;
  last_command_kind_ = kind;
  if (kind == "dropPublisher") {
    runtime_.fault_drop_publisher();
    last_command_result_ = "executed";
  } else if (kind == "manualStopAfterQueuedRestart") {
    std::string error;
    last_command_result_ = runtime_.manual_stop(error) ? "executed" : "failed";
  } else if (kind == "exhaustRetryBudget") {
    runtime_.fault_force_restart_failure();
    last_command_result_ = "armedThreeFailures";
  } else if (kind == "redeliverDuplicateAction") {
    const FaultRedeliveryResult result = runtime_.fault_redeliver_last_action();
    last_command_result_ = result.observed ? "duplicateAcknowledged" : "notObserved";
  } else if (kind == "pauseActionDispatch") {
    last_command_result_ = runtime_.fault_pause_action_dispatch(true) ? "paused"
                                                                      : "failed";
  } else if (kind == "resumeActionDispatch") {
    const ModuleRuntimeSnapshot current = runtime_.snapshot();
    last_command_result_ = runtime_.fault_pause_action_dispatch(false) ? "resumed"
        : current.recovery_desired == "stopped" && !current.recovery_action_pending
              ? "alreadyCancelled" : "failed";
  } else if (kind == "reconnectController" || kind == "letControllerLeaseExpire") {
    runtime_.fault_pause_controller(true);
    last_command_result_ = "paused";
    const uint32_t delay = duration_ms == 0
                               ? (kind == "letControllerLeaseExpire" ? 6000 : 1000)
                               : duration_ms;
    QTimer::singleShot(delay, timer_, [this] {
      if (active_) runtime_.fault_pause_controller(false);
    });
  } else if (kind == "freezePublisher") {
    if (const auto frozen_epoch = runtime_.fault_freeze_publisher()) {
      last_command_result_ = "executed";
      const uint32_t delay = duration_ms == 0 ? 5000 : std::min(duration_ms, 30000u);
      QTimer::singleShot(delay, timer_, [this, epoch = *frozen_epoch] {
        if (active_) runtime_.fault_resume_publisher(epoch);
      });
    } else {
      last_command_result_ = "publisherTransportNotQuiescent";
    }
  } else {
    last_command_result_ = "awaitingHttpEvidence";
  }
  write_report("running");
}

}
