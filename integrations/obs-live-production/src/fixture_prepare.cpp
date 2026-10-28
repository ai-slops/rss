#include "fixture.hpp"

#include "fixture_source.hpp"

#include <algorithm>
#include <utility>

namespace live_production {
namespace {

bool matches_except_passphrase(const PublisherConfig &candidate,
                               const PublisherConfig &primary) {
  return candidate.target.source_id == primary.target.source_id &&
         candidate.target.video_track_id == primary.target.video_track_id &&
         candidate.target.audio_track_id == primary.target.audio_track_id &&
         candidate.target.host == primary.target.host &&
         candidate.target.port == primary.target.port &&
         candidate.target.stream_id == primary.target.stream_id &&
         candidate.target.buffer_ms == primary.target.buffer_ms &&
         candidate.obs.source_name == primary.obs.source_name &&
         candidate.obs.source_uuid == primary.obs.source_uuid &&
         candidate.obs.audio_mix_index == primary.obs.audio_mix_index &&
         candidate.encoding.width == primary.encoding.width &&
         candidate.encoding.height == primary.encoding.height &&
         candidate.encoding.fps_numerator == primary.encoding.fps_numerator &&
         candidate.encoding.fps_denominator == primary.encoding.fps_denominator &&
         candidate.encoding.audio_sample_rate == primary.encoding.audio_sample_rate &&
         candidate.encoding.video_bitrate_kbps == primary.encoding.video_bitrate_kbps;
}

bool matches_bad_auth(const PublisherConfig &candidate,
                      const PublisherConfig &primary) {
  return matches_except_passphrase(candidate, primary) &&
         candidate.target.passphrase != primary.target.passphrase;
}

std::string report_kind(const std::string &command_kind) {
  return command_kind == "startBadAuthSender" ? "badAuth"
                                                : "duplicateSender";
}

}

bool FixtureController::prepare(std::string &error) {
  if (fixture_.sender_paths.size() != control_.sources.size()) {
    error = "fixture sender and source command counts differ";
    return false;
  }
  for (const std::string &path : fixture_.sender_paths) {
    auto command = std::find_if(
        control_.sources.begin(), control_.sources.end(),
        [&](const FixtureSourceCommand &candidate) {
          return candidate.sender_bundle == path;
        });
    if (command == control_.sources.end()) {
      error = "fixture sender has no native source command";
      return false;
    }
    PublisherConfig config;
    if (!load_publisher_config(path, config, error) ||
        !is_loopback_host(config.target.host)) {
      if (error.empty())
        error = "local fixture rejected a non-loopback target";
      return false;
    }
    if (config.target.source_id != command->source_id ||
        config.obs.source_name != command->source_name ||
        config.obs.audio_mix_index != command->audio_mix_index) {
      error = "native source command differs from publisher bundle";
      return false;
    }
    obs_source_t *source = create_fixture_source(*command, config, error);
    if (!source)
      return false;
    fixture_sources_.push_back(source);
    auto publisher = std::make_unique<Publisher>(
        config, "primary-" + config.target.source_id,
        [this] { schedule_evaluate(); });
    primaries_.push_back({*command, std::move(publisher)});
  }
  for (const NegativeCommand &command : control_.negative_cases) {
    PublisherConfig config;
    if (!load_publisher_config(command.sender_bundle, config, error) ||
        !is_loopback_host(config.target.host)) {
      if (error.empty())
        error = "negative fixture target must be loopback";
      return false;
    }
    auto primary = std::find_if(
        primaries_.begin(), primaries_.end(), [&](const PrimarySender &sender) {
          return sender.publisher->config().target.source_id ==
                 config.target.source_id;
        });
    if (primary == primaries_.end()) {
      error = "negative fixture sender has no matching primary";
      return false;
    }
    if (command.kind == "startDuplicateSender") {
      if (std::find(fixture_.sender_paths.begin(), fixture_.sender_paths.end(),
                    command.sender_bundle) == fixture_.sender_paths.end() ||
          !matches_except_passphrase(config, primary->publisher->config()) ||
          config.target.passphrase !=
              primary->publisher->config().target.passphrase) {
        error = "duplicate sender must reuse a primary bundle";
        return false;
      }
    } else if (!matches_bad_auth(config, primary->publisher->config())) {
      error = "bad-auth sender must change only endpoint authentication";
      return false;
    }
    negatives_.push_back({command.kind, std::move(config), nullptr});
  }
  return true;
}

bool FixtureController::start_negative_attempts() {
  negatives_started_ = true;
  for (NegativeAttempt &attempt : negatives_) {
    const std::string instance = report_kind(attempt.kind) + "-" +
                                 attempt.config.target.source_id;
    attempt.publisher = std::make_unique<Publisher>(
        attempt.config, instance, [this] { schedule_evaluate(); });
    std::string error;
    if (!attempt.publisher->initialize(error)) {
      attempt.result = "fixtureError";
      return false;
    }
    if (!attempt.publisher->start(error))
      attempt.result = "rejected";
  }
  return true;
}

}
