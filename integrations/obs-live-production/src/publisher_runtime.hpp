#pragma once

#include "config.hpp"
#include "publisher.hpp"

#include <cstdint>
#include <memory>
#include <optional>
#include <string>

namespace live_production {

class PublisherRuntime {
public:
  bool configure(const PublisherConfig &config, std::string &error);
  void clear();
  bool start(std::string &error);
  void stop();
  bool restart(std::string &error);
  std::optional<uint64_t> freeze_transport() {
    if (!publisher_ || !publisher_->freeze_transport()) return std::nullopt;
    return ++transport_fault_epoch_;
  }
  void resume_transport(uint64_t epoch) {
    if (epoch == transport_fault_epoch_ && publisher_) publisher_->resume_transport();
  }
  void force_restart_failures(uint32_t count) { forced_restart_failures_ = count; }

  std::string state() const;
  std::string last_error() const;
  const std::optional<PublisherConfig> &config() const { return config_; }

private:
  std::optional<PublisherConfig> config_;
  std::unique_ptr<Publisher> publisher_;
  std::string last_state_ = "created";
  std::string last_error_;
  uint64_t instance_id_ = 0;
  uint64_t transport_fault_epoch_ = 0;
  bool transitioning_ = false;
  uint32_t forced_restart_failures_ = 0;
};

}
