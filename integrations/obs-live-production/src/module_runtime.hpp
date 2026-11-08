#pragma once

#include "publisher_runtime.hpp"
#include "fixture_dispatcher.hpp"
#include "recovery_client.hpp"
#include "recovery_action.hpp"
#include "recovery_model.hpp"
#include "recovery_store.hpp"

#include <cstdint>
#include <memory>
#include <optional>
#include <string>
#include <utility>

namespace live_production {

struct ModuleRuntimeSnapshot {
  std::string publisher_state;
  std::string error;
  std::string bundle_path;
  std::optional<PublisherConfig> config;
  bool recovery_enrolled = false;
  bool recovery_online = false;
  bool recovery_ready = false;
  bool start_pending = false;
  std::string recovery_desired = "stopped";
  std::string receiver_state = "unknown";
  std::string retry_result = "never";
  uint32_t retry_attempts = 0;
  uint64_t process_epoch = 0;
  uint64_t intent_epoch = 0;
  uint64_t poll_seq = 0;
  std::string last_command_id;
  std::string last_command_result;
  uint64_t restart_execution_count = 0;
  bool recovery_action_pending = false;
  std::string pending_recovery_command_id;
  std::string duplicate_command_id;
  std::string duplicate_result;
};

struct FaultRedeliveryResult {
  bool observed = false;
  std::string command_id;
  std::string result;
};

class ModuleRuntime {
public:
  explicit ModuleRuntime(std::string recovery_directory = {})
      : recovery_directory_(std::move(recovery_directory)) {}
  ~ModuleRuntime();
  ModuleRuntime(const ModuleRuntime &) = delete;
  ModuleRuntime &operator=(const ModuleRuntime &) = delete;

  bool configure(const std::string &bundle_path, std::string &error);
  bool manual_start(std::string &error);
  bool manual_stop(std::string &error);
  void shutdown();
  ModuleRuntimeSnapshot snapshot() const;

  void fault_drop_publisher();
  std::optional<uint64_t> fault_freeze_publisher();
  void fault_resume_publisher(uint64_t epoch);
  void fault_pause_controller(bool paused);
  void fault_force_restart_failure();
  bool fault_pause_action_dispatch(bool paused);
  FaultRedeliveryResult fault_redeliver_last_action();

private:
  std::optional<PollDispatch> build_poll();
  void handle_response(const PollContext &request,
                       const RecoveryPollResponse &response);
  void handle_client_error(const std::string &error);
  bool persist(std::string &error);
  bool execute_action(const RecoveryAction &action);
  void cancel_pending_action(const std::string &reason);
  void clear_configuration();
  bool perform_manual_start(std::string &error);
  bool ensure_action_dispatcher(std::string &error);
  void run_action(uint64_t intent_epoch);

  PublisherRuntime publisher_;
  RecoveryClient client_;
  RecoveryStore store_;
  std::unique_ptr<RecoveryModel> model_;
  RecoveryActionHandler action_handler_;
  FixtureTaskDispatcher action_dispatcher_;
  std::string bundle_path_;
  std::string runtime_error_;
  bool recovery_online_ = false;
  std::string receiver_state_ = "unknown";
  std::string retry_result_ = "never";
  uint32_t retry_attempts_ = 0;
  bool action_dispatcher_open_ = false;
  RecoveryStartGate start_gate_;
  std::optional<RecoveryPollResponse> last_action_response_;
  std::string duplicate_command_id_;
  std::string duplicate_result_;
  std::string recovery_directory_;
};

}
