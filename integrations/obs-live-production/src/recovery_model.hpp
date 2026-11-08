#pragma once

#include <cstdint>
#include <optional>
#include <string>

namespace live_production {

enum class LocalDesired { Running, Stopped };
enum class ActionOutcome { Started, Failed };
enum class ActionDecision { Reject, Execute, DuplicateStarted, DuplicateFailed };

struct PersistedRecoveryState {
  uint64_t process_epoch = 0;
  uint64_t poll_seq = 0;
  uint64_t intent_epoch = 0;
  LocalDesired desired = LocalDesired::Stopped;
  std::string last_command_id;
  std::optional<ActionOutcome> last_command_result;
};

struct RecoveryAction {
  std::string kind;
  std::string command_id;
  std::string server_run_id;
  std::string controller_id;
  uint64_t process_epoch = 0;
  uint64_t intent_epoch = 0;
  uint32_t attempt = 0;
};

struct RecoveryFence {
  std::string source_id;
  std::string controller_id;
  std::string server_run_id;
  uint64_t process_epoch = 0;
  uint64_t poll_seq = 0;
  uint64_t intent_epoch = 0;
  bool source_enabled = false;
  bool policy_enabled = false;
  bool maintenance = false;
};

class RecoveryModel {
public:
  explicit RecoveryModel(PersistedRecoveryState state);

  const PersistedRecoveryState &state() const { return state_; }
  bool begin_process();
  bool manual_start();
  bool manual_stop();
  std::optional<uint64_t> next_poll();
  ActionDecision decide(const RecoveryFence &fence, const RecoveryAction &action,
                        const std::string &source_id,
                        const std::string &controller_id) const;
  void persist_command(const std::string &command_id);
  void complete_command(ActionOutcome outcome);

private:
  PersistedRecoveryState state_;
};

class RecoveryStartGate {
public:
  bool request_start();
  bool register_stopped();
  void cancel();
  bool ready() const { return ready_; }
  bool pending() const { return pending_; }

private:
  bool ready_ = false;
  bool pending_ = false;
};

const char *desired_name(LocalDesired value);
const char *outcome_name(ActionOutcome value);

}
