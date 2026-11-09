#include "report.hpp"
#include "private_files_windows.hpp"

#include <obs.h>

#include <cstdio>
#include <fstream>
#include <sys/stat.h>

namespace live_production {
namespace {

std::string json_escape(const std::string &value) {
  std::string escaped;
  escaped.reserve(value.size());
  for (const char character : value) {
    switch (character) {
    case '\\': escaped += "\\\\"; break;
    case '"': escaped += "\\\""; break;
    case '\n': escaped += "\\n"; break;
    case '\r': escaped += "\\r"; break;
    case '\t': escaped += "\\t"; break;
    default:
      if (static_cast<unsigned char>(character) >= 0x20)
        escaped += character;
    }
  }
  return escaped;
}

}

bool write_report(const std::string &path,
                  const std::string &fixture_state,
                  const std::vector<SenderEvidence> &senders,
                  const std::vector<NegativeEvidence> &negative_cases,
                  std::string &error) {
  const std::string temporary = path + ".tmp";
  std::ofstream output(temporary, std::ios::trunc);
  if (!output) {
    error = "cannot create fixture report";
    return false;
  }
  output << "{\"schemaVersion\":1,\"moduleLoaded\":true,"
            "\"module\":\"obs-live-production\",\"obsVersion\":\""
         << json_escape(obs_get_version_string()) << "\",\"state\":\""
         << json_escape(fixture_state) << "\",\"senders\":[";
  for (size_t index = 0; index < senders.size(); ++index) {
    if (index != 0)
      output << ',';
    const SenderEvidence &sender = senders[index];
    output << "{\"sourceId\":\"" << json_escape(sender.source_id)
           << "\",\"sourceName\":\"" << json_escape(sender.source_name)
           << "\",\"audioMixIndex\":" << sender.audio_mix_index
           << ",\"state\":\"" << json_escape(sender.state)
           << "\",\"error\":\"" << json_escape(sender.error)
           << "\",\"visual\":{\"color\":\""
           << json_escape(sender.color) << "\",\"frameCounter\":"
           << (sender.frame_counter ? "true" : "false")
           << "},\"audio\":{\"toneHz\":" << sender.tone_hz << "}}";
  }
  output << "],\"negativeCases\":[";
  for (size_t index = 0; index < negative_cases.size(); ++index) {
    if (index != 0)
      output << ',';
    output << "{\"kind\":\"" << json_escape(negative_cases[index].kind)
           << "\",\"result\":\""
           << json_escape(negative_cases[index].result) << "\"}";
  }
  output << "]}\n";
  output.close();
  if (!output) {
    error = "cannot finalize fixture report";
    return false;
  }
#ifdef _WIN32
  if (!secure_windows_path(temporary)) {
#else
  if (chmod(temporary.c_str(), 0600) != 0) {
#endif
    remove(temporary.c_str());
    error = "cannot secure fixture report";
    return false;
  }
#ifdef _WIN32
  if (!MoveFileExW(QString::fromStdString(temporary).toStdWString().c_str(),
                   QString::fromStdString(path).toStdWString().c_str(), MOVEFILE_REPLACE_EXISTING)) {
#else
  if (rename(temporary.c_str(), path.c_str()) != 0) {
#endif
    remove(temporary.c_str());
    error = "cannot publish fixture report";
    return false;
  }
  return true;
}

}
