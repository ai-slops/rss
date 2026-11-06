#include "live_test_output.hpp"

#include "live_test_receiver.hpp"

#include <stdexcept>

namespace {
void require(bool value, const char *message) {
  if (!value) throw std::runtime_error(message);
}
}

LiveTestOutput::LiveTestOutput(obs_source_t *source, LiveTestReceiver &receiver,
                               const std::string &name, const char *passphrase,
                               uint32_t fps_num, uint32_t fps_den) {
  obs_video_info video{};
  require(obs_get_video_info(&video), "video info unavailable");
  video.base_width = video.output_width = 64;
  video.base_height = video.output_height = 64;
  view_ = obs_view_create();
  view_video_ = obs_view_add2(view_, &video);
  require(view_ && view_video_, "view creation failed");
  obs_view_set_source(view_, 0, source);
  obs_data_t *video_settings = obs_data_create();
  obs_data_set_string(video_settings, "rate_control", "CBR");
  obs_data_set_int(video_settings, "bitrate", 500);
  obs_data_set_int(video_settings, "keyint_sec", 1);
  obs_data_set_string(video_settings, "preset", "ultrafast");
  video_encoder_ = obs_video_encoder_create("obs_x264", (name + "-video").c_str(),
                                            video_settings, nullptr);
  obs_data_release(video_settings);
  obs_data_t *audio_settings = obs_data_create();
  obs_data_set_int(audio_settings, "bitrate", 128);
  audio_encoder_ = obs_audio_encoder_create("ffmpeg_aac", (name + "-audio").c_str(),
                                            audio_settings, 0, nullptr);
  obs_data_release(audio_settings);
  require(video_encoder_ && audio_encoder_, "encoder creation failed");
  obs_encoder_set_video(video_encoder_, view_video_);
  obs_encoder_set_audio(audio_encoder_, obs_get_audio());
  obs_data_t *settings = obs_data_create();
  obs_data_set_string(settings, "host", "127.0.0.1");
  obs_data_set_int(settings, "port", receiver.port());
  obs_data_set_string(settings, "passphrase", passphrase);
  obs_data_set_string(settings, "streamId", name.c_str());
  obs_data_set_int(settings, "bufferMs", 120);
  obs_data_set_int(settings, "width", 64);
  obs_data_set_int(settings, "height", 64);
  obs_data_set_int(settings, "fpsNumerator", fps_num);
  obs_data_set_int(settings, "fpsDenominator", fps_den);
  output_ = obs_output_create("live_production_mpegts_srt", name.c_str(), settings, nullptr);
  obs_data_release(settings);
  require(output_, "output creation failed");
  obs_output_set_video_encoder(output_, video_encoder_);
  obs_output_set_audio_encoder(output_, audio_encoder_, 0);
  signal_handler_t *signals = obs_output_get_signal_handler(output_);
  signal_handler_connect(signals, "start", started, this);
  signal_handler_connect(signals, "stop", stopped, this);
}

LiveTestOutput::~LiveTestOutput() {
  stop();
  if (output_) {
    signal_handler_t *signals = obs_output_get_signal_handler(output_);
    signal_handler_disconnect(signals, "start", started, this);
    signal_handler_disconnect(signals, "stop", stopped, this);
    obs_output_release(output_);
  }
  if (audio_encoder_) obs_encoder_release(audio_encoder_);
  if (video_encoder_) obs_encoder_release(video_encoder_);
  if (view_) {
    if (view_video_) obs_view_remove(view_);
    obs_view_destroy(view_);
  }
}

bool LiveTestOutput::start() { return obs_output_start(output_); }

void LiveTestOutput::stop() {
  if (output_ && obs_output_active(output_)) obs_output_stop(output_);
}

int LiveTestOutput::freeze() {
  calldata_t params;
  calldata_init(&params);
  const bool called = proc_handler_call(obs_output_get_proc_handler(output_),
                                        "live_production_freeze", &params);
  const bool success = called && calldata_bool(&params, "success");
  const int epoch = static_cast<int>(calldata_int(&params, "epoch"));
  calldata_free(&params);
  return success ? epoch : 0;
}

bool LiveTestOutput::resume(int epoch) {
  calldata_t params;
  calldata_init(&params);
  calldata_set_int(&params, "epoch", epoch);
  const bool called = proc_handler_call(obs_output_get_proc_handler(output_),
                                        "live_production_resume", &params);
  const bool success = called && calldata_bool(&params, "success");
  calldata_free(&params);
  return success;
}

void LiveTestOutput::started(void *data, calldata_t *) {
  static_cast<LiveTestOutput *>(data)->starts_.fetch_add(1);
}

void LiveTestOutput::stopped(void *data, calldata_t *params) {
  auto *self = static_cast<LiveTestOutput *>(data);
  self->stop_code_.store(static_cast<int>(calldata_int(params, "code")));
  self->stops_.fetch_add(1);
}
