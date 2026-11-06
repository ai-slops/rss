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
    if (!publisher_ || !publisher_->freeze_transport()) {
      active_transport_fault_.reset();
      return std::nullopt;
    }
    if (!active_transport_fault_) active_transport_fault_ = ++transport_fault_epoch_;
    return active_transport_fault_;
  }
  void resume_transport(uint64_t epoch) {
    if (active_transport_fault_ == epoch && publisher_ && publisher_->resume_transport())
      active_transport_fault_.reset();
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
  std::optional<uint64_t> active_transport_fault_;
  bool transitioning_ = false;
  uint32_t forced_restart_failures_ = 0;
};

}
