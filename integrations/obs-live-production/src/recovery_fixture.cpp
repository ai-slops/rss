#include "recovery_fixture.hpp"
#include "private_files_windows.hpp"

#include "config.hpp"
#include "fixture_source.hpp"
#include "module_runtime.hpp"
#include "recovery_fault.hpp"

#include <obs-data.h>

#include <QDateTime>
#include <QObject>
#include <QTimer>
#include <QUrl>

#include <algorithm>
#include <cstdio>
#include <cerrno>
#include <cstdlib>
#include <fstream>
#include <sys/stat.h>
#include <utility>

namespace live_production {
namespace {

std::string text(obs_data_t *data, const char *key) {
  const char *value = obs_data_get_string(data, key);
  return value ? value : "";
}

std::string escape(const std::string &value) {
  std::string result;
  for (char character : value) {
    if (character == '\\' || character == '"') result += '\\';
    result += character;
  }
  return result;
}

bool loopback_url(const std::string &value) {
  const QUrl url(QString::fromStdString(value));
  return url.isValid() && (url.host() == "127.0.0.1" || url.host() == "localhost" ||
                           url.host() == "::1");
}

bool command_id(const std::string &value, uint64_t &result) {
  if (value.empty() || value.find_first_not_of("0123456789") != std::string::npos)
    return false;
  errno = 0;
  char *end = nullptr;
  const unsigned long long parsed = std::strtoull(value.c_str(), &end, 10);
  if (errno != 0 || !end || *end != '\0' || parsed == 0) return false;
  result = static_cast<uint64_t>(parsed);
  return true;
}

}

RecoveryFixture::RecoveryFixture(ModuleRuntime &runtime) : runtime_(runtime) {
  timer_ = new QTimer();
  timer_->setSingleShot(true);
  QObject::connect(timer_, &QTimer::timeout, [this] { execute_next(); });
}

RecoveryFixture::~RecoveryFixture() {
  stop();
  delete timer_;
}

bool RecoveryFixture::start_from_environment(std::string &error) {
  const char *path = std::getenv("LIVE_PRODUCTION_OBS_RECOVERY_FIXTURE_FILE");
  if (!path || !*path) return true;
  if (!is_private_regular_file(path, error)) return false;
  obs_data_t *root = obs_data_create_from_json_file(path);
  if (!root) {
    error = "recovery fixture is not valid JSON";
    return false;
  }
  source_id_ = text(root, "sourceId");
  bundle_path_ = text(root, "publisherBundle");
  report_file_ = text(root, "reportFile");
  command_file_ = text(root, "commandFile");
  const uint64_t deadline_seconds = static_cast<uint64_t>(obs_data_get_int(root, "deadlineSeconds"));
  obs_data_array_t *items = obs_data_get_array(root, "operations");
  const size_t count = items ? obs_data_array_count(items) : 0;
  for (size_t index = 0; index < count; ++index) {
    obs_data_t *item = obs_data_array_item(items, index);
    RecoveryFaultOperation operation{text(item, "kind"),
                                     static_cast<uint32_t>(obs_data_get_int(item, "durationMs"))};
    obs_data_release(item);
    if (std::find(recovery_fault_kinds().begin(), recovery_fault_kinds().end(),
                  operation.kind) == recovery_fault_kinds().end()) {
      error = "recovery fixture contains unsupported operation";
      break;
    }
    operations_.push_back(std::move(operation));
  }
  if (items) obs_data_array_release(items);
  const bool valid = obs_data_get_int(root, "schemaVersion") == 1 &&
                     obs_data_get_bool(root, "localFixtureOnly") && !source_id_.empty() &&
                     !bundle_path_.empty() && !report_file_.empty() &&
                     !command_file_.empty() && deadline_seconds > 0 && deadline_seconds <= 600 &&
                     !operations_.empty();
  obs_data_release(root);
  if (!error.empty() || !valid) {
    if (error.empty()) error = "recovery fixture has missing fields";
    return false;
  }
  PublisherConfig bundle;
  if (!load_publisher_config(bundle_path_, bundle, error) ||
      bundle.target.source_id != source_id_ || !bundle.recovery ||
      !is_loopback_host(bundle.target.host) ||
      !loopback_url(bundle.recovery->control_url))
    return false;
  FixtureSourceCommand source{bundle_path_, source_id_, bundle.obs.source_name,
                              "red", true, 440, bundle.obs.audio_mix_index};
  fixture_source_ = create_fixture_source(source, bundle, error);
  if (!fixture_source_) return false;
  if (!runtime_.configure(bundle_path_, error)) {
    release_fixture_source(fixture_source_);
    fixture_source_ = nullptr;
    return false;
  }
  active_ = true;
  deadline_ms_ = static_cast<uint64_t>(QDateTime::currentMSecsSinceEpoch()) +
                 deadline_seconds * 1000;
  write_report("registering");
  timer_->start(100);
  return true;
}

void RecoveryFixture::execute_next() {
  if (!active_) return;
  if (static_cast<uint64_t>(QDateTime::currentMSecsSinceEpoch()) >= deadline_ms_) {
    write_report("deadlineExceeded");
    active_ = false;
    std::string error;
    runtime_.manual_stop(error);
    if (fixture_source_) release_fixture_source(fixture_source_);
    fixture_source_ = nullptr;
    return;
  }
  const ModuleRuntimeSnapshot snapshot = runtime_.snapshot();
  if (!start_requested_) {
    if (!snapshot.recovery_ready) {
      write_report("registering");
      timer_->start(100);
      return;
    }
    std::string error;
    if (!runtime_.manual_start(error)) {
      last_command_result_ = "manualStartFailed";
      write_report("failed");
      return;
    }
    start_requested_ = true;
    write_report("waitingForMedia");
  }
  std::string command_error;
  if (is_private_regular_file(command_file_, command_error)) {
    obs_data_t *command = obs_data_create_from_json_file(command_file_.c_str());
    if (command) {
      const std::string id = text(command, "commandId");
      const std::string kind = text(command, "kind");
      const uint32_t duration = static_cast<uint32_t>(obs_data_get_int(command, "durationMs"));
      uint64_t parsed_id = 0;
      obs_data_release(command);
      if (command_id(id, parsed_id) && parsed_id > last_command_id_)
        execute_command(parsed_id, kind, duration);
    }
  }
  write_report("running");
  timer_->start(250);
}

void RecoveryFixture::write_report(const std::string &state) {
  std::ofstream output(report_file_ + ".tmp", std::ios::trunc);
  if (!output) return;
  const ModuleRuntimeSnapshot snapshot = runtime_.snapshot();
  output << "{\"schemaVersion\":1,\"sourceId\":\"" << escape(source_id_)
         << "\",\"state\":\"" << state << "\",\"publisherState\":\""
         << snapshot.publisher_state << "\",\"recoveryReady\":"
         << (snapshot.recovery_ready ? "true" : "false")
         << ",\"localDesired\":\"" << snapshot.recovery_desired
         << "\",\"processEpoch\":\"" << snapshot.process_epoch
         << "\",\"intentEpoch\":\"" << snapshot.intent_epoch
         << "\",\"pollSeq\":\"" << snapshot.poll_seq
         << "\",\"lastRecoveryCommandId\":\"" << escape(snapshot.last_command_id)
         << "\",\"lastRecoveryResult\":\"" << escape(snapshot.last_command_result)
         << "\",\"restartExecutionCount\":" << snapshot.restart_execution_count
         << ",\"recoveryActionPending\":"
         << (snapshot.recovery_action_pending ? "true" : "false")
         << ",\"pendingRecoveryCommandId\":\""
         << escape(snapshot.pending_recovery_command_id)
         << "\",\"duplicateCommandId\":\"" << escape(snapshot.duplicate_command_id)
         << "\",\"duplicateResult\":\"" << escape(snapshot.duplicate_result)
          << "\",\"executedCommandId\":\"" << last_command_id_
         << "\",\"executedCommandKind\":\"" << escape(last_command_kind_)
         << "\",\"executedCommandResult\":\"" << escape(last_command_result_)
         << "\",\"receiverState\":\"" << snapshot.receiver_state << "\"}\n";
  output.close();
#ifdef _WIN32
  const auto temporary = report_file_ + ".tmp";
  if (secure_windows_path(temporary))
    MoveFileExW(QString::fromStdString(temporary).toStdWString().c_str(),
                QString::fromStdString(report_file_).toStdWString().c_str(), MOVEFILE_REPLACE_EXISTING);
#else
  chmod((report_file_ + ".tmp").c_str(), 0600);
  rename((report_file_ + ".tmp").c_str(), report_file_.c_str());
#endif
}

void RecoveryFixture::stop() {
  if (!active_ && !fixture_source_) return;
  active_ = false;
  timer_->stop();
  std::string error;
  runtime_.manual_stop(error);
  write_report("stopped");
  if (fixture_source_) release_fixture_source(fixture_source_);
  fixture_source_ = nullptr;
}

}
