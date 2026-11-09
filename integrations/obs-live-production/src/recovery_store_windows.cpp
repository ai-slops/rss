#include "recovery_store.hpp"
#include "private_files_windows.hpp"

#include <QDir>
#include <QFile>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLockFile>
#include <QRegularExpression>
#include <QSaveFile>
#include <algorithm>
#include <cctype>

namespace live_production {

class RecoveryStore::WindowsLock {
 public:
  explicit WindowsLock(const QString &path) : file(path) { file.setStaleLockTime(0); }
  QLockFile file;
};

RecoveryStore::RecoveryStore() = default;
RecoveryStore::~RecoveryStore() { close(); }

bool RecoveryStore::open(const std::string &directory, const std::string &bundle_path,
                         const std::string &source_id, const std::string &controller_id,
                         PersistedRecoveryState &state, std::string &error) {
  close();
  const bool safe_id = !source_id.empty() && source_id.size() <= 64 &&
      std::all_of(source_id.begin(), source_id.end(), [](unsigned char value) {
        return std::isalnum(value) || value == '-';
      });
  const QString dir = QString::fromStdString(directory);
  if (!safe_id || (!QDir(dir).exists() &&
      (!QDir().mkpath(dir) || !secure_windows_path(directory))) ||
      !private_windows_path(directory, true, error)) {
    error = "recovery state directory must be private";
    return false;
  }
  windows_ = std::make_unique<WindowsLock>(dir + '/' + QString::fromStdString(source_id) + ".lock");
  if (!windows_->file.tryLock(0)) {
    close(); error = "recovery state is already owned by another OBS process"; return false;
  }
  state_path_ = directory + "/" + source_id + ".json";
  bundle_path_ = bundle_path;
  source_id_ = source_id;
  controller_id_ = controller_id;
  if (!QFile::exists(QString::fromStdString(state_path_))) return true;
  if (!private_windows_path(state_path_, false, error)) { close(); return false; }
  QFile input(QString::fromStdString(state_path_));
  if (!input.open(QIODevice::ReadOnly) || input.size() > 65536) {
    close(); error = "cannot read private recovery state"; return false;
  }
  const auto document = QJsonDocument::fromJson(input.readAll());
  const auto json = document.object();
  const auto string = [&](const char *key) { return json.value(key).toString(); };
  bool valid = document.isObject() && string("sourceId").toStdString() == source_id_ &&
      (string("desired") == "running" || string("desired") == "stopped");
  const auto number = [&](const char *key, uint64_t &value) {
    const auto text = string(key); bool ok = false;
    value = text.toULongLong(&ok);
    return ok && !text.isEmpty() && text.indexOf(QRegularExpression("[^0-9]")) < 0;
  };
  valid = number("processEpoch", state.process_epoch) && valid;
  valid = number("pollSeq", state.poll_seq) && valid;
  valid = number("intentEpoch", state.intent_epoch) && valid;
  if (!valid || !json.contains("bundlePath") || !json.contains("controllerId")) {
    close(); error = "recovery state does not match selected bundle"; return false;
  }
  const bool same_controller = string("controllerId").toStdString() == controller_id_;
  state.desired = same_controller && string("desired") == "running" ? LocalDesired::Running : LocalDesired::Stopped;
  state.last_command_id = string("lastCommandId").toStdString();
  const auto result = string("lastCommandResult");
  if (result == "started") state.last_command_result = ActionOutcome::Started;
  else if (result == "failed") state.last_command_result = ActionOutcome::Failed;
  else state.last_command_result.reset();
  if (!same_controller || string("bundlePath").toStdString() != bundle_path_) {
    state.last_command_id.clear(); state.last_command_result.reset();
  }
  return true;
}

bool RecoveryStore::save(const PersistedRecoveryState &state, std::string &error) const {
  if (!windows_ || !windows_->file.isLocked()) { error = "recovery state is not locked"; return false; }
  QJsonObject json{{"schemaVersion", 1}, {"bundlePath", QString::fromStdString(bundle_path_)},
      {"sourceId", QString::fromStdString(source_id_)}, {"controllerId", QString::fromStdString(controller_id_)},
      {"processEpoch", QString::number(state.process_epoch)}, {"pollSeq", QString::number(state.poll_seq)},
      {"intentEpoch", QString::number(state.intent_epoch)}, {"desired", QString::fromLatin1(desired_name(state.desired))},
      {"lastCommandId", QString::fromStdString(state.last_command_id)},
      {"lastCommandResult", QString::fromLatin1(state.last_command_result ? outcome_name(*state.last_command_result) : "")}};
  QSaveFile file(QString::fromStdString(state_path_));
  const QByteArray bytes = QJsonDocument(json).toJson(QJsonDocument::Compact);
  if (!file.open(QIODevice::WriteOnly) || file.write(bytes) != bytes.size() || !file.commit() ||
      !private_windows_path(state_path_, false, error)) {
    error = "cannot persist private recovery state"; return false;
  }
  return true;
}

void RecoveryStore::close() { windows_.reset(); state_path_.clear(); }

}
