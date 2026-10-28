#include "config.hpp"

#include <obs-data.h>

#include <limits>
#include <sys/stat.h>

namespace live_production {
namespace {

std::string text(obs_data_t *data, const char *key) {
  const char *value = obs_data_get_string(data, key);
  return value ? value : "";
}

bool positive_u32(obs_data_t *data, const char *key, uint32_t &value) {
  const long long raw = obs_data_get_int(data, key);
  if (raw <= 0 ||
      static_cast<unsigned long long>(raw) > std::numeric_limits<uint32_t>::max())
    return false;
  value = static_cast<uint32_t>(raw);
  return true;
}

bool require_text(const std::string &value, const char *field,
                  std::string &error) {
  if (!value.empty())
    return true;
  error = std::string("missing required field: ") + field;
  return false;
}

}

bool is_private_regular_file(const std::string &path, std::string &error) {
  struct stat info {};
  if (lstat(path.c_str(), &info) != 0) {
    error = "cannot stat private configuration file";
    return false;
  }
  if (!S_ISREG(info.st_mode)) {
    error = "configuration path is not a regular file";
    return false;
  }
  if ((info.st_mode & 0077) != 0) {
    error = "configuration file must not grant group or other permissions";
    return false;
  }
  return true;
}

bool load_publisher_config(const std::string &path, PublisherConfig &config,
                           std::string &error) {
  if (!is_private_regular_file(path, error))
    return false;
  obs_data_t *root = obs_data_create_from_json_file(path.c_str());
  if (!root) {
    error = "publisher bundle is not valid JSON";
    return false;
  }
  config.schema_version = static_cast<uint32_t>(obs_data_get_int(root, "schemaVersion"));
  obs_data_t *target = obs_data_get_obj(root, "target");
  obs_data_t *obs = obs_data_get_obj(root, "obs");
  obs_data_t *encoding = obs_data_get_obj(root, "encoding");
  const auto release_data = [&] {
    if (encoding)
      obs_data_release(encoding);
    if (obs)
      obs_data_release(obs);
    if (target)
      obs_data_release(target);
    obs_data_release(root);
  };
  if (!target || !obs || !encoding) {
    error = "publisher bundle requires target, obs, and encoding objects";
    release_data();
    return false;
  }

  config.target.source_id = text(target, "sourceId");
  config.target.video_track_id = text(target, "videoTrackId");
  config.target.audio_track_id = text(target, "audioTrackId");
  config.target.host = text(target, "host");
  config.target.passphrase = text(target, "passphrase");
  config.target.stream_id = text(target, "streamId");
  config.obs.source_name = text(obs, "sourceName");
  config.obs.source_uuid = text(obs, "sourceUuid");

  uint32_t port = 0;
  const long long mix = obs_data_get_int(obs, "audioMixIndex");
  const bool numbers_ok = positive_u32(target, "port", port) &&
                          port <= std::numeric_limits<uint16_t>::max() &&
                          positive_u32(target, "bufferMs", config.target.buffer_ms) &&
                          positive_u32(encoding, "width", config.encoding.width) &&
                          positive_u32(encoding, "height", config.encoding.height) &&
                          positive_u32(encoding, "fpsNumerator", config.encoding.fps_numerator) &&
                          positive_u32(encoding, "fpsDenominator", config.encoding.fps_denominator) &&
                          positive_u32(encoding, "audioSampleRate", config.encoding.audio_sample_rate) &&
                          positive_u32(encoding, "videoBitrateKbps", config.encoding.video_bitrate_kbps);
  config.target.port = static_cast<uint16_t>(port);
  config.obs.audio_mix_index = static_cast<uint32_t>(mix);

  const bool passphrase_ok = config.target.passphrase.size() >= 10 &&
                             config.target.passphrase.size() <= 79;
  const bool host_ok = config.target.host.find_first_of("/?&# \t\r\n") ==
                       std::string::npos;
  if (config.schema_version != 1 || !numbers_ok ||
      !obs_data_has_user_value(obs, "audioMixIndex") || mix < 0 || mix > 5 ||
      !passphrase_ok || !host_ok ||
      config.encoding.audio_sample_rate != 48000) {
    error = "publisher bundle has unsupported schema or numeric values";
    release_data();
    return false;
  }
  if (!require_text(config.target.source_id, "target.sourceId", error) ||
      !require_text(config.target.video_track_id, "target.videoTrackId", error) ||
      !require_text(config.target.audio_track_id, "target.audioTrackId", error) ||
      !require_text(config.target.host, "target.host", error) ||
      !require_text(config.target.passphrase, "target.passphrase", error) ||
      !require_text(config.target.stream_id, "target.streamId", error) ||
      !require_text(config.obs.source_name, "obs.sourceName", error)) {
    release_data();
    return false;
  }

  release_data();
  return true;
}

bool is_loopback_host(const std::string &host) {
  return host == "127.0.0.1" || host == "localhost" || host == "::1";
}

}
