#include "recovery_store.hpp"

#include <sys/stat.h>
#include <unistd.h>

#include <filesystem>
#include <iostream>
#include <stdexcept>

namespace {

void require(bool value, const char *message) {
  if (!value) throw std::runtime_error(message);
}

void private_state_roundtrips_and_excludes_second_owner() {
  const std::filesystem::path base =
      std::filesystem::temp_directory_path() /
      ("obs-recovery-store-" + std::to_string(getpid()));
  const std::filesystem::path root = base / "plugin_config" / "publisher" / "recovery";
  std::filesystem::remove_all(base);
  live_production::RecoveryStore first;
  live_production::PersistedRecoveryState state;
  std::string error;
  require(first.open(root.string(), "/private/bundle.json", "source", "controller",
                     state, error),
          "first store open failed");
  live_production::RecoveryModel model(state);
  require(model.begin_process(), "process epoch did not advance");
  require(model.manual_start(), "manual start did not advance intent");
  model.persist_command("command");
  model.complete_command(live_production::ActionOutcome::Started);
  require(first.save(model.state(), error), "state save failed");
  live_production::RecoveryStore second;
  live_production::PersistedRecoveryState blocked;
  require(!second.open(root.string(), "/private/bundle.json", "source", "controller",
                       blocked, error),
          "second store acquired process lock");
  first.close();
  live_production::PersistedRecoveryState loaded;
  require(second.open(root.string(), "/private/bundle.json", "source", "controller",
                      loaded, error),
          "stored state reload failed");
  require(loaded.process_epoch == 1 && loaded.intent_epoch == 1 &&
              loaded.desired == live_production::LocalDesired::Running &&
              loaded.last_command_id == "command" && loaded.last_command_result ==
                                                       live_production::ActionOutcome::Started,
          "stored state values changed");
  struct stat info {};
  require(stat((root / "source.json").c_str(), &info) == 0 &&
              (info.st_mode & 0077) == 0,
          "state file permissions are not private");
  second.close();
  live_production::RecoveryStore rotated;
  live_production::PersistedRecoveryState rotated_state;
  require(rotated.open(root.string(), "/private/rotated.json", "source",
                       "controller-two", rotated_state, error),
          "rotated controller could not reuse source high-water state");
  require(rotated_state.process_epoch == 1 &&
              rotated_state.desired == live_production::LocalDesired::Stopped &&
              rotated_state.last_command_id.empty(),
          "rotated controller inherited running intent or command");
  rotated.close();
  std::filesystem::remove_all(base);
}

}

int main() {
  try {
    private_state_roundtrips_and_excludes_second_owner();
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
