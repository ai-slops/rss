#include "recovery_protocol.hpp"

#include <QByteArray>

#include <iostream>
#include <stdexcept>

namespace {

void require(bool value, const char *message) {
  if (!value) throw std::runtime_error(message);
}

void exact_poll_response_parses_all_fences() {
  const QByteArray json = R"json({
    "schemaVersion":1,"sourceId":"source","policyVersion":"1","sourceEnabled":true,
    "policy":{"enabled":true,"maintenance":false},
    "controller":{"enrolled":true,"controllerId":"controller","online":true,"processEpoch":"2","localDesired":"running"},
    "receiver":{"state":"stalled","continuityGeneration":"4","videoFrames":"10","audioBuffers":"20","lastVideoPtsNs":"30"},
    "retry":{"attempts":1,"maximumAttempts":3,"nextEligibleAtNs":null,"lastResult":"pending"},
    "capabilities":{"publisherRestart":true,"processRestart":false,"freezeDetection":"decoded_counter_and_pts_progress"},
    "serverRunId":"run","processEpoch":"2","pollSeq":"9","intentEpoch":"7","nextPollMs":1000,
    "action":{"kind":"restartPublisher","commandId":"command","serverRunId":"run","controllerId":"controller","processEpoch":"2","intentEpoch":"7","attempt":1}
  })json";
  live_production::RecoveryPollResponse response;
  std::string error;
  require(live_production::parse_poll_response(json, response, error),
          "valid response was rejected");
  require(response.fence.source_id == "source" && response.fence.policy_enabled &&
              response.controller_online && response.controller_local_desired == "running" &&
              response.receiver_state == "stalled" &&
              response.retry_attempts == 1 && response.action &&
              response.action->command_id == "command" && response.echoed.poll_seq == 9,
          "response fields changed while parsing");
}

void malformed_or_unfenced_action_is_rejected() {
  const QByteArray json = R"json({"schemaVersion":1,"sourceId":"source",
    "policy":{"enabled":true,"maintenance":false},"sourceEnabled":true,
    "controller":{"controllerId":"controller","online":true},
    "serverRunId":"run","processEpoch":"2","pollSeq":"9","intentEpoch":"7",
    "nextPollMs":1000,"action":{"kind":"restartPublisher"}})json";
  live_production::RecoveryPollResponse response;
  std::string error;
  require(!live_production::parse_poll_response(json, response, error),
          "unfenced action was accepted");
}

}

int main() {
  try {
    exact_poll_response_parses_all_fences();
    malformed_or_unfenced_action_is_rejected();
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
