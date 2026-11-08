#pragma once

#include "recovery_model.hpp"

#include <cstdint>
#include <optional>
#include <string>

class QByteArray;

namespace live_production {

struct PollContext {
  uint64_t process_epoch = 0;
  uint64_t poll_seq = 0;
  uint64_t intent_epoch = 0;
};

struct RecoveryPollResponse {
  RecoveryFence fence;
  PollContext echoed;
  uint32_t next_poll_ms = 1000;
  std::optional<RecoveryAction> action;
  bool controller_online = false;
  std::string controller_local_desired = "stopped";
  std::string receiver_state = "unknown";
  uint32_t retry_attempts = 0;
  std::string retry_result = "never";
};

bool parse_poll_response(const QByteArray &bytes, RecoveryPollResponse &response,
                         std::string &error);

}
