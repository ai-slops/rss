#pragma once

#include "recovery_model.hpp"

#include <string>
#include <memory>

namespace live_production {

class RecoveryStore {
public:
  RecoveryStore();
  ~RecoveryStore();
  RecoveryStore(const RecoveryStore &) = delete;
  RecoveryStore &operator=(const RecoveryStore &) = delete;

  bool open(const std::string &directory, const std::string &bundle_path,
            const std::string &source_id, const std::string &controller_id,
            PersistedRecoveryState &state, std::string &error);
  bool save(const PersistedRecoveryState &state, std::string &error) const;
  void close();

private:
  int lock_fd_ = -1;
  std::string state_path_;
  std::string bundle_path_;
  std::string source_id_;
  std::string controller_id_;
#ifdef _WIN32
  class WindowsLock;
  std::unique_ptr<WindowsLock> windows_;
#endif
};

}
