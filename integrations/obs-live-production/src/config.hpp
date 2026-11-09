#pragma once

#include <cstdint>
#include <optional>
#include <string>
#include <vector>

namespace live_production {

struct TargetConfig {
  std::string source_id;
  std::string video_track_id;
  std::string audio_track_id;
  std::string host;
  uint16_t port = 0;
  std::string passphrase;
  std::string stream_id;
  uint32_t buffer_ms = 0;
};

struct ObsConfig {
  std::string source_name;
  std::string source_uuid;
  uint32_t audio_mix_index = 0;
  bool source_audio = false;
};

struct EncodingConfig {
  uint32_t width = 0;
  uint32_t height = 0;
  uint32_t fps_numerator = 0;
  uint32_t fps_denominator = 0;
  uint32_t audio_sample_rate = 0;
  uint32_t video_bitrate_kbps = 0;
};

struct RecoveryConfig {
  uint32_t schema_version = 0;
  std::string control_url;
  std::string controller_id;
  std::string controller_token;
};

struct PublisherConfig {
  uint32_t schema_version = 0;
  TargetConfig target;
  ObsConfig obs;
  EncodingConfig encoding;
  std::optional<RecoveryConfig> recovery;
};

struct FixtureConfig {
  uint32_t schema_version = 0;
  bool local_fixture_only = false;
  std::vector<std::string> sender_paths;
  std::string report_file;
  std::string control_file;
  uint32_t deadline_seconds = 0;
};

struct FixtureSourceCommand {
  std::string sender_bundle;
  std::string source_id;
  std::string source_name;
  std::string color;
  bool frame_counter = false;
  uint32_t tone_hz = 0;
  uint32_t audio_mix_index = 0;
};

struct NegativeCommand {
  std::string kind;
  std::string sender_bundle;
  std::string expected;
};

struct ControlConfig {
  uint32_t schema_version = 0;
  std::vector<FixtureSourceCommand> sources;
  std::vector<NegativeCommand> negative_cases;
};

bool load_publisher_config(const std::string &path, PublisherConfig &config,
                           std::string &error);
bool load_fixture_config(const std::string &path, FixtureConfig &config,
                         std::string &error);
bool load_control_config(const std::string &path, ControlConfig &config,
                         std::string &error);
bool is_private_regular_file(const std::string &path, std::string &error);
bool is_loopback_host(const std::string &host);

}
