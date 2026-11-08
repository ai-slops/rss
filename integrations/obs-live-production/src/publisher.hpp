#pragma once

#include "config.hpp"

#include <obs.h>

#include <functional>
#include <mutex>
#include <cstdint>
#include <optional>
#include <string>

namespace live_production {

class Publisher {
public:
  Publisher(PublisherConfig config, std::string instance_name,
            std::function<void()> state_changed);
  ~Publisher();

  Publisher(const Publisher &) = delete;
  Publisher &operator=(const Publisher &) = delete;

  bool initialize(std::string &error);
  bool start(std::string &error);
  void stop();
  bool freeze_transport();
  bool resume_transport();

  const PublisherConfig &config() const { return config_; }
  std::string state() const;
  std::string last_error() const;

private:
  static void output_starting(void *context, calldata_t *);
  static void output_started(void *context, calldata_t *);
  static void output_stopping(void *context, calldata_t *);
  static void output_stopped(void *context, calldata_t *data);
  static void output_reconnecting(void *context, calldata_t *);
  void set_state(std::string state, std::string error = {});
  void release();

  PublisherConfig config_;
  std::string instance_name_;
  std::function<void()> state_changed_;
  mutable std::mutex state_mutex_;
  std::string state_ = "created";
  std::string last_error_;
  obs_source_t *source_ = nullptr;
  obs_view_t *view_ = nullptr;
  video_t *video_ = nullptr;
  obs_encoder_t *video_encoder_ = nullptr;
  obs_encoder_t *audio_encoder_ = nullptr;
  obs_output_t *output_ = nullptr;
  std::optional<int64_t> transport_epoch_;
};

}
