#include "recovery_fault.hpp"

#include <algorithm>

namespace live_production {

const std::vector<std::string> &recovery_fault_kinds() {
  static const std::vector<std::string> kinds = {
      "dropPublisher", "freezePublisher", "manualStopAfterQueuedRestart",
      "pollMissingAuthorization", "pollWrongSourceAuthorization",
      "exhaustRetryBudget", "reconnectController", "redeliverDuplicateAction",
      "observeServerRestart", "setMaintenance", "setSourceDisabled",
      "letControllerLeaseExpire", "pauseActionDispatch", "resumeActionDispatch"};
  return kinds;
}

bool accept_recovery_fault(uint64_t previous_id, uint64_t command_id,
                           const std::string &kind,
                           const std::vector<std::string> &allowlist) {
  return command_id > previous_id &&
         std::find(recovery_fault_kinds().begin(), recovery_fault_kinds().end(), kind) !=
             recovery_fault_kinds().end() &&
         std::find(allowlist.begin(), allowlist.end(), kind) != allowlist.end();
}

}
