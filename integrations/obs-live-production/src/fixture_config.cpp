#include "config.hpp"

#include <obs-data.h>

#include <cctype>
#include <fstream>
#include <sstream>
#include <utility>

namespace live_production {
namespace {

std::string text(obs_data_t *data, const char *key) {
  const char *value = obs_data_get_string(data, key);
  return value ? value : "";
}

void skip_space(const std::string &json, size_t &position) {
  while (position < json.size() &&
         std::isspace(static_cast<unsigned char>(json[position])))
    ++position;
}

bool parse_string(const std::string &json, size_t &position,
                  std::string &value) {
  if (position >= json.size() || json[position++] != '"')
    return false;
  while (position < json.size()) {
    const char character = json[position++];
    if (character == '"')
      return true;
    if (character != '\\') {
      value += character;
      continue;
    }
    if (position >= json.size())
      return false;
    const char escaped = json[position++];
    if (escaped == '"' || escaped == '\\' || escaped == '/')
      value += escaped;
    else if (escaped == 'b')
      value += '\b';
    else if (escaped == 'f')
      value += '\f';
    else if (escaped == 'n')
      value += '\n';
    else if (escaped == 'r')
      value += '\r';
    else if (escaped == 't')
      value += '\t';
    else
      return false;
  }
  return false;
}

bool parse_sender_paths(const std::string &path,
                        std::vector<std::string> &senders,
                        std::string &error) {
  std::ifstream input(path);
  std::ostringstream buffer;
  buffer << input.rdbuf();
  if (!input) {
    error = "cannot read fixture file";
    return false;
  }
  const std::string json = buffer.str();
  size_t position = json.find("\"senders\"");
  if (position == std::string::npos) {
    error = "fixture file omitted senders";
    return false;
  }
  position = json.find(':', position + 9);
  if (position == std::string::npos)
    return false;
  ++position;
  skip_space(json, position);
  if (position >= json.size() || json[position++] != '[')
    return false;
  while (true) {
    skip_space(json, position);
    if (position < json.size() && json[position] == ']')
      break;
    std::string sender;
    if (!parse_string(json, position, sender) || sender.empty()) {
      error = "fixture senders must be non-empty string paths";
      return false;
    }
    senders.push_back(std::move(sender));
    skip_space(json, position);
    if (position < json.size() && json[position] == ',') {
      ++position;
      continue;
    }
    if (position >= json.size() || json[position] != ']')
      return false;
  }
  return !senders.empty();
}

bool parse_source_command(obs_data_t *item, FixtureSourceCommand &command) {
  command.sender_bundle = text(item, "senderBundle");
  command.source_id = text(item, "sourceId");
  command.source_name = text(item, "sourceName");
  command.color = text(item, "color");
  command.frame_counter = obs_data_get_bool(item, "frameCounter");
  const long long tone = obs_data_get_int(item, "toneHz");
  const long long mix = obs_data_get_int(item, "audioMixIndex");
  command.tone_hz = static_cast<uint32_t>(tone);
  command.audio_mix_index = static_cast<uint32_t>(mix);
  return !command.sender_bundle.empty() && !command.source_id.empty() &&
         !command.source_name.empty() &&
         (command.color == "red" || command.color == "blue") &&
         command.frame_counter && tone > 0 &&
         obs_data_has_user_value(item, "audioMixIndex") && mix >= 0 && mix <= 5;
}

}

bool load_fixture_config(const std::string &path, FixtureConfig &config,
                         std::string &error) {
  if (!is_private_regular_file(path, error) ||
      !parse_sender_paths(path, config.sender_paths, error))
    return false;
  obs_data_t *root = obs_data_create_from_json_file(path.c_str());
  if (!root) {
    error = "fixture file is not valid JSON";
    return false;
  }
  config.schema_version = static_cast<uint32_t>(obs_data_get_int(root, "schemaVersion"));
  config.local_fixture_only = obs_data_get_bool(root, "localFixtureOnly");
  config.report_file = text(root, "reportFile");
  config.control_file = text(root, "controlFile");
  config.deadline_seconds = static_cast<uint32_t>(obs_data_get_int(root, "deadlineSeconds"));
  obs_data_release(root);
  if (config.schema_version != 1 || !config.local_fixture_only ||
      config.report_file.empty() || config.control_file.empty() ||
      config.deadline_seconds == 0 || config.deadline_seconds > 600) {
    error = "fixture file has unsupported or missing values";
    return false;
  }
  return true;
}

bool load_control_config(const std::string &path, ControlConfig &config,
                         std::string &error) {
  if (!is_private_regular_file(path, error))
    return false;
  obs_data_t *root = obs_data_create_from_json_file(path.c_str());
  if (!root) {
    error = "control file is not valid JSON";
    return false;
  }
  config.schema_version = static_cast<uint32_t>(obs_data_get_int(root, "schemaVersion"));
  obs_data_array_t *commands = obs_data_get_array(root, "commands");
  const size_t count = commands ? obs_data_array_count(commands) : 0;
  for (size_t index = 0; index < count; ++index) {
    obs_data_t *item = obs_data_array_item(commands, index);
    const std::string kind = text(item, "kind");
    if (kind == "configureNativeFixtureSource") {
      FixtureSourceCommand command;
      if (!parse_source_command(item, command)) {
        error = "invalid native fixture source command";
        obs_data_release(item);
        break;
      }
      config.sources.push_back(std::move(command));
    } else if (kind == "startBadAuthSender" ||
               kind == "startDuplicateSender") {
      config.negative_cases.push_back(
          {kind, text(item, "senderBundle"), text(item, "expected")});
    } else {
      error = "unsupported fixture command";
    }
    obs_data_release(item);
    if (!error.empty())
      break;
  }
  if (commands)
    obs_data_array_release(commands);
  obs_data_release(root);
  if (!error.empty())
    return false;
  if (config.schema_version != 1 || config.sources.empty() ||
      config.negative_cases.size() != 2) {
    error = "control file has unsupported or missing commands";
    return false;
  }
  bool has_bad_auth = false;
  bool has_duplicate = false;
  for (const NegativeCommand &command : config.negative_cases) {
    if (command.sender_bundle.empty() || command.expected != "rejected") {
      error = "negative fixture command must expect rejection";
      return false;
    }
    has_bad_auth = has_bad_auth || command.kind == "startBadAuthSender";
    has_duplicate = has_duplicate || command.kind == "startDuplicateSender";
  }
  if (!has_bad_auth || !has_duplicate) {
    error = "control file requires bad-auth and duplicate-sender commands";
    return false;
  }
  return true;
}

}
