#include "recovery_store.hpp"

#include <fcntl.h>
#include <sys/file.h>
#include <sys/stat.h>
#include <unistd.h>

#include <cstdio>
#include <algorithm>
#include <cctype>
#include <fstream>
#include <filesystem>
#include <sstream>

namespace live_production {
namespace {

std::string escape_json(const std::string &value) {
  std::string output;
  for (const char character : value) {
    if (character == '\\' || character == '"') output += '\\';
    output += character;
  }
  return output;
}

std::optional<std::string> json_string(const std::string &json,
                                       const std::string &key) {
  const std::string token = "\"" + key + "\":\"";
  size_t position = json.find(token);
  if (position == std::string::npos) return std::nullopt;
  position += token.size();
  std::string output;
  bool escaped = false;
  for (; position < json.size(); ++position) {
    const char character = json[position];
    if (escaped) {
      output += character;
      escaped = false;
    } else if (character == '\\') {
      escaped = true;
    } else if (character == '"') {
      return output;
    } else {
      output += character;
    }
  }
  return std::nullopt;
}

std::optional<uint64_t> decimal(const std::string &json,
                                const std::string &key) {
  const auto value = json_string(json, key);
  if (!value || value->empty() || value->find_first_not_of("0123456789") != std::string::npos)
    return std::nullopt;
  try {
    return std::stoull(*value);
  } catch (...) {
    return std::nullopt;
  }
}

bool ensure_private_directory(const std::string &path) {
  struct stat info {};
  if (lstat(path.c_str(), &info) != 0) {
    std::error_code error;
    if (!std::filesystem::create_directories(path, error) || error ||
        chmod(path.c_str(), 0700) != 0)
      return false;
    if (lstat(path.c_str(), &info) != 0) return false;
  }
  return S_ISDIR(info.st_mode) && info.st_uid == geteuid() &&
         (info.st_mode & 0077) == 0;
}

bool safe_id(const std::string &value) {
  return !value.empty() && value.size() <= 64 &&
         std::all_of(value.begin(), value.end(), [](unsigned char character) {
           return std::isalnum(character) || character == '-';
         });
}

bool write_all(int fd, const std::string &value) {
  size_t offset = 0;
  while (offset < value.size()) {
    const ssize_t written = write(fd, value.data() + offset, value.size() - offset);
    if (written <= 0) return false;
    offset += static_cast<size_t>(written);
  }
  return true;
}

}

RecoveryStore::~RecoveryStore() { close(); }

bool RecoveryStore::open(const std::string &directory,
                         const std::string &bundle_path,
                         const std::string &source_id,
                         const std::string &controller_id,
                         PersistedRecoveryState &state, std::string &error) {
  close();
  if (!ensure_private_directory(directory) || !safe_id(source_id)) {
    error = "recovery state directory must be private";
    return false;
  }
  const std::string lock_path = directory + "/" + source_id + ".lock";
  lock_fd_ = ::open(lock_path.c_str(), O_CREAT | O_RDWR | O_CLOEXEC | O_NOFOLLOW, 0600);
  if (lock_fd_ < 0 || flock(lock_fd_, LOCK_EX | LOCK_NB) != 0) {
    close();
    error = "recovery state is already owned by another OBS process";
    return false;
  }
  state_path_ = directory + "/" + source_id + ".json";
  bundle_path_ = bundle_path;
  source_id_ = source_id;
  controller_id_ = controller_id;
  struct stat state_info {};
  if (lstat(state_path_.c_str(), &state_info) != 0) return true;
  if (!S_ISREG(state_info.st_mode) || (state_info.st_mode & 0077) != 0) {
    error = "recovery state file must be private and regular";
    close();
    return false;
  }
  std::ifstream input(state_path_);
  if (!input) {
    error = "cannot read private recovery state";
    close();
    return false;
  }
  std::ostringstream bytes;
  bytes << input.rdbuf();
  const std::string json = bytes.str();
  const auto stored_bundle = json_string(json, "bundlePath");
  const auto stored_source = json_string(json, "sourceId");
  const auto stored_controller = json_string(json, "controllerId");
  const auto process = decimal(json, "processEpoch");
  const auto poll = decimal(json, "pollSeq");
  const auto intent = decimal(json, "intentEpoch");
  const auto desired = json_string(json, "desired");
  if (!stored_bundle || !stored_source || !stored_controller || !process || !poll ||
      !intent || !desired || *stored_source != source_id_ ||
      (*desired != "running" && *desired != "stopped")) {
    error = "recovery state does not match selected bundle";
    close();
    return false;
  }
  state.process_epoch = *process;
  state.poll_seq = *poll;
  state.intent_epoch = *intent;
  const bool same_controller = *stored_controller == controller_id_;
  state.desired = same_controller && *desired == "running" ? LocalDesired::Running
                                                            : LocalDesired::Stopped;
  state.last_command_id = json_string(json, "lastCommandId").value_or("");
  const auto result = json_string(json, "lastCommandResult");
  if (result == "started") state.last_command_result = ActionOutcome::Started;
  else if (result == "failed") state.last_command_result = ActionOutcome::Failed;
  else if (result && !result->empty()) {
    error = "recovery state has invalid command result";
    close();
    return false;
  }
  if (!same_controller || *stored_bundle != bundle_path_) {
    state.last_command_id.clear();
    state.last_command_result.reset();
  }
  return true;
}

bool RecoveryStore::save(const PersistedRecoveryState &state,
                         std::string &error) const {
  if (lock_fd_ < 0) {
    error = "recovery state is not locked";
    return false;
  }
  const std::string temporary = state_path_ + ".tmp." + std::to_string(getpid()) + "." +
                                std::to_string(state.process_epoch) + "." +
                                std::to_string(state.poll_seq) + "." +
                                std::to_string(state.intent_epoch);
  const int fd = ::open(temporary.c_str(), O_CREAT | O_EXCL | O_WRONLY | O_CLOEXEC | O_NOFOLLOW,
                        0600);
  if (fd < 0) {
    error = "cannot create recovery state";
    return false;
  }
  const std::string result = state.last_command_result
                                 ? outcome_name(*state.last_command_result)
                                 : "";
  const std::string json =
      "{\"schemaVersion\":1,\"bundlePath\":\"" + escape_json(bundle_path_) +
      "\",\"sourceId\":\"" + escape_json(source_id_) + "\",\"controllerId\":\"" +
      escape_json(controller_id_) + "\",\"processEpoch\":\"" +
      std::to_string(state.process_epoch) + "\",\"pollSeq\":\"" +
      std::to_string(state.poll_seq) + "\",\"intentEpoch\":\"" +
      std::to_string(state.intent_epoch) + "\",\"desired\":\"" +
      desired_name(state.desired) + "\",\"lastCommandId\":\"" +
      escape_json(state.last_command_id) + "\",\"lastCommandResult\":\"" + result + "\"}\n";
  bool ok = write_all(fd, json);
  if (ok) ok = fsync(fd) == 0;
  if (::close(fd) != 0) ok = false;
  if (ok) ok = rename(temporary.c_str(), state_path_.c_str()) == 0;
  if (ok) {
    const size_t separator = state_path_.find_last_of('/');
    const std::string directory = separator == std::string::npos
                                      ? "."
                                      : state_path_.substr(0, separator);
    const int directory_fd = ::open(directory.c_str(), O_RDONLY | O_CLOEXEC | O_DIRECTORY);
    ok = directory_fd >= 0 && fsync(directory_fd) == 0;
    if (directory_fd >= 0) ::close(directory_fd);
  }
  if (!ok) {
    std::remove(temporary.c_str());
    error = "cannot persist recovery state";
    return false;
  }
  if (chmod(state_path_.c_str(), 0600) != 0) {
    error = "cannot secure recovery state";
    return false;
  }
  return true;
}

void RecoveryStore::close() {
  if (lock_fd_ >= 0) {
    flock(lock_fd_, LOCK_UN);
    ::close(lock_fd_);
  }
  lock_fd_ = -1;
  state_path_.clear();
}

}
