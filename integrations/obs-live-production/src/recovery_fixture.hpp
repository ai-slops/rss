#pragma once

#include <cstdint>
#include <string>
#include <vector>

#include <obs.h>

class QTimer;

namespace live_production {

class ModuleRuntime;

struct RecoveryFaultOperation {
  std::string kind;
  uint32_t duration_ms = 0;
  std::string result = "pending";
};

class RecoveryFixture {
public:
  explicit RecoveryFixture(ModuleRuntime &runtime);
  ~RecoveryFixture();

  bool start_from_environment(std::string &error);
  void stop();

private:
  void execute_next();
  void execute_command(uint64_t command_id, const std::string &kind,
                       uint32_t duration_ms);
  void write_report(const std::string &state);

  ModuleRuntime &runtime_;
  QTimer *timer_ = nullptr;
  std::string source_id_;
  std::string bundle_path_;
  std::string report_file_;
  std::string command_file_;
  std::vector<RecoveryFaultOperation> operations_;
  obs_source_t *fixture_source_ = nullptr;
  uint64_t last_command_id_ = 0;
  uint64_t deadline_ms_ = 0;
  std::string last_command_kind_;
  std::string last_command_result_;
  bool start_requested_ = false;
  bool active_ = false;
};

}
