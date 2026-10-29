#include "module_runtime.hpp"

#include <obs-module.h>

#include <utility>

namespace live_production {

ModuleRuntime::~ModuleRuntime() { shutdown(); }

bool ModuleRuntime::configure(const std::string &bundle_path,
                              std::string &error) {
  clear_configuration();
  PublisherConfig config;
  if (!load_publisher_config(bundle_path, config, error) ||
      !publisher_.configure(config, error))
    return false;
  bundle_path_ = bundle_path;
  runtime_error_.clear();
  recovery_online_ = false;
  receiver_state_ = "unknown";
  retry_result_ = "never";
  retry_attempts_ = 0;
  start_gate_.cancel();
  last_action_response_.reset();
  duplicate_command_id_.clear();
  duplicate_result_.clear();
  if (!config.recovery) return true;
  std::string directory = recovery_directory_;
  if (directory.empty()) {
    char *configured = obs_module_config_path("recovery");
    if (!configured) {
      error = "cannot resolve private recovery state directory";
      clear_configuration();
      return false;
    }
    directory = configured;
    bfree(configured);
  }
  PersistedRecoveryState state;
  const bool opened = store_.open(directory, bundle_path, config.target.source_id,
                                  config.recovery->controller_id, state, error);
  if (!opened) {
    clear_configuration();
    return false;
  }
  model_ = std::make_unique<RecoveryModel>(std::move(state));
  if (!model_->begin_process()) {
    error = "recovery process epoch is exhausted";
    model_.reset();
    store_.close();
    publisher_.clear();
    bundle_path_.clear();
    return false;
  }
  if (!persist(error)) {
    model_.reset();
    store_.close();
    publisher_.clear();
    bundle_path_.clear();
    return false;
  }
  if (!ensure_action_dispatcher(error) ||
      !client_.configure(
          *config.recovery, [this] { return build_poll(); },
          [this](const PollContext &request, const RecoveryPollResponse &response) {
            handle_response(request, response);
          },
          [this](const std::string &failure) { handle_client_error(failure); }, error)) {
    clear_configuration();
    return false;
  }
  client_.start();
  return true;
}

void ModuleRuntime::clear_configuration() {
  cancel_pending_action("configuration change cancelled pending recovery action");
  client_.stop();
  action_dispatcher_.close();
  action_dispatcher_open_ = false;
  action_handler_.cancel();
  store_.close();
  model_.reset();
  publisher_.clear();
  bundle_path_.clear();
  runtime_error_.clear();
  recovery_online_ = false;
  receiver_state_ = "unknown";
  retry_result_ = "never";
  retry_attempts_ = 0;
  start_gate_.cancel();
  last_action_response_.reset();
  duplicate_command_id_.clear();
  duplicate_result_.clear();
}

bool ModuleRuntime::manual_start(std::string &error) {
  if (model_) {
    if (!ensure_action_dispatcher(error)) return false;
    if (!start_gate_.request_start()) {
      error = "manual Start queued until controller registration completes";
      return true;
    }
  }
  return perform_manual_start(error);
}

bool ModuleRuntime::perform_manual_start(std::string &error) {
  if (model_) {
    if (!model_->manual_start()) {
      error = "recovery intent epoch is exhausted";
      runtime_error_ = error;
      return false;
    }
    if (!persist(error)) {
      model_->manual_stop();
      runtime_error_ = error;
      return false;
    }
  }
  const bool started = publisher_.start(error);
  if (!started) runtime_error_ = error;
  return started;
}

bool ModuleRuntime::manual_stop(std::string &error) {
  bool persisted = true;
  if (model_) {
    cancel_pending_action("manual Stop cancelled pending recovery action");
    model_->manual_stop();
    client_.stop();
    action_dispatcher_.close();
    action_dispatcher_open_ = false;
    action_handler_.cancel();
    start_gate_.cancel();
    persisted = persist(error);
  }
  publisher_.stop();
  if (model_) client_.start();
  if (!persisted) runtime_error_ = error;
  return persisted;
}

void ModuleRuntime::shutdown() {
  std::string error;
  if (model_) {
    cancel_pending_action("OBS shutdown cancelled pending recovery action");
    if (model_->state().desired != LocalDesired::Stopped)
      model_->manual_stop();
    client_.stop();
    if (!persist(error)) runtime_error_ = error;
  } else {
    client_.stop();
  }
  publisher_.stop();
  action_dispatcher_.close();
  action_dispatcher_open_ = false;
  action_handler_.cancel();
  model_.reset();
  store_.close();
}

ModuleRuntimeSnapshot ModuleRuntime::snapshot() const {
  ModuleRuntimeSnapshot result;
  result.publisher_state = publisher_.state();
  result.error = runtime_error_.empty() ? publisher_.last_error() : runtime_error_;
  result.bundle_path = bundle_path_;
  result.config = publisher_.config();
  result.recovery_enrolled = model_ != nullptr;
  result.recovery_online = recovery_online_;
  result.recovery_ready = start_gate_.ready();
  result.start_pending = start_gate_.pending();
  result.recovery_desired = model_ ? desired_name(model_->state().desired) : "stopped";
  result.receiver_state = receiver_state_;
  result.retry_result = retry_result_;
  result.retry_attempts = retry_attempts_;
  result.process_epoch = model_ ? model_->state().process_epoch : 0;
  result.intent_epoch = model_ ? model_->state().intent_epoch : 0;
  result.poll_seq = model_ ? model_->state().poll_seq : 0;
  result.last_command_id = model_ ? model_->state().last_command_id : "";
  result.last_command_result =
      model_ && model_->state().last_command_result
          ? outcome_name(*model_->state().last_command_result)
          : "";
  result.restart_execution_count = action_handler_.execution_count();
  result.recovery_action_pending = action_handler_.pending();
  result.pending_recovery_command_id =
      action_handler_.pending() && model_ ? model_->state().last_command_id : "";
  result.duplicate_command_id = duplicate_command_id_;
  result.duplicate_result = duplicate_result_;
  return result;
}

bool ModuleRuntime::persist(std::string &error) {
  return model_ && store_.save(model_->state(), error);
}

}
