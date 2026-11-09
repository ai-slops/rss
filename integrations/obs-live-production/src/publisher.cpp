#include "publisher.hpp"
#include "source_audio.hpp"

#include <obs-module.h>

#include <utility>

namespace live_production {
namespace {

obs_data_t *video_settings(const EncodingConfig &encoding) {
  obs_data_t *settings = obs_data_create();
  obs_data_set_string(settings, "rate_control", "CBR");
  obs_data_set_int(settings, "bitrate", encoding.video_bitrate_kbps);
  obs_data_set_int(settings, "keyint_sec", 2);
  obs_data_set_string(settings, "preset", "veryfast");
  obs_data_set_string(settings, "profile", "high");
  obs_data_set_bool(settings, "repeat_headers", true);
  return settings;
}

obs_data_t *audio_settings() {
  obs_data_t *settings = obs_data_create();
  obs_data_set_int(settings, "bitrate", 160);
  obs_data_set_bool(settings, "set_to_ADTS", true);
  return settings;
}

obs_data_t *output_settings(const PublisherConfig &config) {
  obs_data_t *settings = obs_data_create();
  obs_data_set_string(settings, "host", config.target.host.c_str());
  obs_data_set_int(settings, "port", config.target.port);
  obs_data_set_string(settings, "passphrase", config.target.passphrase.c_str());
  obs_data_set_string(settings, "streamId", config.target.stream_id.c_str());
  obs_data_set_int(settings, "bufferMs", config.target.buffer_ms);
  obs_data_set_int(settings, "width", config.encoding.width);
  obs_data_set_int(settings, "height", config.encoding.height);
  obs_data_set_int(settings, "fpsNumerator", config.encoding.fps_numerator);
  obs_data_set_int(settings, "fpsDenominator", config.encoding.fps_denominator);
  return settings;
}

}

Publisher::Publisher(PublisherConfig config, std::string instance_name,
                     std::function<void()> state_changed)
    : config_(std::move(config)), instance_name_(std::move(instance_name)),
      state_changed_(std::move(state_changed)) {}

Publisher::~Publisher() { release(); }

bool Publisher::initialize(std::string &error) {
  const auto fail = [this, &error](const char *message) {
    error = message;
    set_state("failed", error);
    return false;
  };
  source_ = config_.obs.source_uuid.empty()
                ? obs_get_source_by_name(config_.obs.source_name.c_str())
                : obs_get_source_by_uuid(config_.obs.source_uuid.c_str());
  if (!source_)
    return fail("configured OBS source was not found");

  obs_video_info info {};
  if (!obs_get_video_info(&info))
    return fail("OBS video is not initialized");
  obs_audio_info audio_info {};
  if (!obs_get_audio_info(&audio_info) || audio_info.samples_per_sec != 48000 ||
      audio_info.speakers != SPEAKERS_STEREO)
    return fail("OBS audio must be configured for 48 kHz stereo");
  info.base_width = config_.encoding.width;
  info.base_height = config_.encoding.height;
  info.output_width = config_.encoding.width;
  info.output_height = config_.encoding.height;
  info.fps_num = config_.encoding.fps_numerator;
  info.fps_den = config_.encoding.fps_denominator;
  view_ = obs_view_create();
  video_ = view_ ? obs_view_add2(view_, &info) : nullptr;
  if (!video_)
    return fail("failed to create independent OBS video view");
  obs_view_set_source(view_, 0, source_);

  obs_data_t *video_config = video_settings(config_.encoding);
  const std::string video_name = "live-production-video-" + instance_name_;
  video_encoder_ = obs_video_encoder_create("obs_x264", video_name.c_str(),
                                            video_config, nullptr);
  obs_data_release(video_config);
  obs_data_t *audio_config = audio_settings();
  const std::string audio_name = "live-production-audio-" + instance_name_;
  audio_encoder_ = obs_audio_encoder_create(
      "ffmpeg_aac", audio_name.c_str(), audio_config,
      config_.obs.source_audio ? 0 : config_.obs.audio_mix_index, nullptr);
  obs_data_release(audio_config);
  if (!video_encoder_ || !audio_encoder_)
    return fail("required obs_x264 or ffmpeg_aac encoder is unavailable");
  obs_encoder_set_video(video_encoder_, video_);
  if (config_.obs.source_audio) {
    source_audio_ = std::make_unique<SourceAudio>(source_);
    if (!source_audio_->initialize()) return fail("cannot initialize independent source audio");
    obs_encoder_set_audio(audio_encoder_, source_audio_->output());
  } else {
    obs_encoder_set_audio(audio_encoder_, obs_get_audio());
  }

  obs_data_t *settings = output_settings(config_);
  const std::string output_name = "live-production-output-" + instance_name_;
  output_ = obs_output_create("live_production_mpegts_srt", output_name.c_str(),
                              settings, nullptr);
  obs_data_release(settings);
  if (!output_)
    return fail("owned MPEG-TS SRT output is unavailable");
  obs_output_set_reconnect_settings(output_, 0, 0);
  obs_output_set_video_encoder(output_, video_encoder_);
  obs_output_set_audio_encoder(output_, audio_encoder_, 0);

  signal_handler_t *signals = obs_output_get_signal_handler(output_);
  signal_handler_connect(signals, "starting", output_starting, this);
  signal_handler_connect(signals, "start", output_started, this);
  signal_handler_connect(signals, "stopping", output_stopping, this);
  signal_handler_connect(signals, "stop", output_stopped, this);
  signal_handler_connect(signals, "reconnect", output_reconnecting, this);
  set_state("ready");
  return true;
}

bool Publisher::start(std::string &error) {
  if (obs_output_start(output_))
    return true;
  error = "OBS rejected output start; inspect local OBS logs privately";
  set_state("failed", error);
  return false;
}

void Publisher::stop() {
  resume_transport();
  if (output_ && obs_output_active(output_))
    obs_output_stop(output_);
}

bool Publisher::freeze_transport() {
  if (!output_ || !obs_output_active(output_)) return false;
  calldata_t params;
  calldata_init(&params);
  const bool called = proc_handler_call(obs_output_get_proc_handler(output_),
                                       "live_production_freeze", &params);
  const int64_t epoch = calldata_int(&params, "epoch");
  const bool frozen = called && calldata_bool(&params, "success") && epoch > 0;
  if (frozen) transport_epoch_ = epoch;
  calldata_free(&params);
  return frozen;
}

bool Publisher::resume_transport() {
  if (!output_ || !transport_epoch_) return false;
  calldata_t params;
  calldata_init(&params);
  calldata_set_int(&params, "epoch", *transport_epoch_);
  const bool called = proc_handler_call(obs_output_get_proc_handler(output_),
                                       "live_production_resume", &params);
  const bool resumed = called && calldata_bool(&params, "success");
  if (resumed) transport_epoch_.reset();
  calldata_free(&params);
  return resumed;
}

std::string Publisher::state() const {
  std::lock_guard<std::mutex> lock(state_mutex_);
  return state_;
}

std::string Publisher::last_error() const {
  std::lock_guard<std::mutex> lock(state_mutex_);
  return last_error_;
}

void Publisher::output_starting(void *context, calldata_t *) {
  static_cast<Publisher *>(context)->set_state("starting");
}

void Publisher::output_started(void *context, calldata_t *) {
  static_cast<Publisher *>(context)->set_state("sending");
}

void Publisher::output_stopping(void *context, calldata_t *) {
  static_cast<Publisher *>(context)->set_state("stopping");
}

void Publisher::output_stopped(void *context, calldata_t *data) {
  auto *publisher = static_cast<Publisher *>(context);
  const long long code = calldata_int(data, "code");
  publisher->set_state(code == OBS_OUTPUT_SUCCESS ? "stopped" : "failed",
                       code == OBS_OUTPUT_SUCCESS ? "" : "OBS output stopped with an error");
}

void Publisher::output_reconnecting(void *context, calldata_t *) {
  static_cast<Publisher *>(context)->set_state("reconnecting");
}

void Publisher::set_state(std::string state, std::string error) {
  {
    std::lock_guard<std::mutex> lock(state_mutex_);
    state_ = std::move(state);
    last_error_ = std::move(error);
  }
  if (state_changed_)
    state_changed_();
}

void Publisher::release() {
  stop();
  if (output_) {
    signal_handler_t *signals = obs_output_get_signal_handler(output_);
    signal_handler_disconnect(signals, "starting", output_starting, this);
    signal_handler_disconnect(signals, "start", output_started, this);
    signal_handler_disconnect(signals, "stopping", output_stopping, this);
    signal_handler_disconnect(signals, "stop", output_stopped, this);
    signal_handler_disconnect(signals, "reconnect", output_reconnecting, this);
    obs_output_release(output_);
    output_ = nullptr;
  }
  if (audio_encoder_)
    obs_encoder_release(audio_encoder_);
  if (video_encoder_)
    obs_encoder_release(video_encoder_);
  source_audio_.reset();
  if (view_) {
    if (video_)
      obs_view_remove(view_);
    obs_view_destroy(view_);
  }
  if (source_)
    obs_source_release(source_);
  audio_encoder_ = nullptr;
  video_encoder_ = nullptr;
  view_ = nullptr;
  video_ = nullptr;
  source_ = nullptr;
}

}
