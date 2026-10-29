#include "publisher_runtime.hpp"

namespace live_production {

bool PublisherRuntime::configure(const PublisherConfig &config,
                                 std::string &error) {
  const std::string current = state();
  if (current == "starting" || current == "sending" || current == "reconnecting" ||
      current == "stopping") {
    error = "stop the publisher before selecting another bundle";
    return false;
  }
  publisher_.reset();
  config_ = config;
  last_state_ = "ready";
  last_error_.clear();
  return true;
}

void PublisherRuntime::clear() {
  stop();
  config_.reset();
  last_state_ = "created";
  last_error_.clear();
}

bool PublisherRuntime::start(std::string &error) {
  if (!config_ || transitioning_) {
    error = "publisher is not ready";
    return false;
  }
  transitioning_ = true;
  if (!publisher_ || publisher_->state() == "failed") {
    publisher_.reset();
    publisher_ = std::make_unique<Publisher>(
        *config_, "runtime-" + std::to_string(++instance_id_), [] {});
    if (!publisher_->initialize(error)) {
      last_state_ = "failed";
      last_error_ = error;
      transitioning_ = false;
      return false;
    }
  }
  const bool started = publisher_->start(error);
  last_state_ = publisher_->state();
  last_error_ = started ? "" : error;
  transitioning_ = false;
  return started;
}

void PublisherRuntime::stop() {
  transitioning_ = true;
  if (publisher_) publisher_->stop();
  publisher_.reset();
  last_state_ = config_ ? "stopped" : "created";
  transitioning_ = false;
}

bool PublisherRuntime::restart(std::string &error) {
  if (forced_restart_failures_ != 0) {
    stop();
    error = "forced fixture restart failure";
    last_state_ = "failed";
    last_error_ = error;
    --forced_restart_failures_;
    return false;
  }
  stop();
  return start(error);
}

std::string PublisherRuntime::state() const {
  if (publisher_) return publisher_->state();
  return last_state_;
}

std::string PublisherRuntime::last_error() const {
  if (publisher_) return publisher_->last_error();
  return last_error_;
}

}
