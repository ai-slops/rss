#include "fixture_source.hpp"

#include <util/platform.h>

#include <algorithm>
#include <atomic>
#include <cmath>
#include <cstdint>
#include <new>
#include <thread>
#include <vector>

namespace live_production {
namespace {

constexpr uint32_t audio_sample_rate = 48000;
constexpr uint32_t audio_packet_frames = 1024;
constexpr double two_pi = 6.28318530717958647692;

class FixtureSource {
public:
  FixtureSource(obs_source_t *source, obs_data_t *settings)
      : source_(source),
        width_(static_cast<uint32_t>(obs_data_get_int(settings, "width"))),
        height_(static_cast<uint32_t>(obs_data_get_int(settings, "height"))),
        fps_numerator_(static_cast<uint32_t>(obs_data_get_int(settings, "fpsNumerator"))),
        fps_denominator_(static_cast<uint32_t>(obs_data_get_int(settings, "fpsDenominator"))),
        tone_hz_(static_cast<uint32_t>(obs_data_get_int(settings, "toneHz"))),
        color_(std::string(obs_data_get_string(settings, "color")) == "red"
                   ? 0xFFFF0000u
                   : 0xFF0000FFu),
        pixels_(static_cast<size_t>(width_) * height_),
        samples_(audio_packet_frames * 2) {}

  ~FixtureSource() {
    stop_.store(true);
    if (thread_.joinable())
      thread_.join();
  }

  bool start() {
    try {
      thread_ = std::thread([this] { run(); });
      return true;
    } catch (...) {
      return false;
    }
  }

private:
  void fill_video() {
    std::fill(pixels_.begin(), pixels_.end(), color_);
    const uint32_t band_top = height_ - std::max(1u, height_ / 5);
    for (uint32_t bit = 0; bit < 16; ++bit) {
      const uint32_t left = bit * width_ / 16;
      const uint32_t right = (bit + 1) * width_ / 16;
      const uint32_t value = (frame_counter_ & (1u << bit)) != 0
                                 ? 0xFFFFFFFFu
                                 : 0xFF000000u;
      for (uint32_t y = band_top; y < height_; ++y)
        std::fill(pixels_.begin() + static_cast<size_t>(y) * width_ + left,
                  pixels_.begin() + static_cast<size_t>(y) * width_ + right,
                  value);
    }
    ++frame_counter_;
  }

  void fill_audio() {
    const double step = two_pi * tone_hz_ / audio_sample_rate;
    for (uint32_t frame = 0; frame < audio_packet_frames; ++frame) {
      const float sample = static_cast<float>(std::sin(phase_) * 0.25);
      samples_[frame * 2] = sample;
      samples_[frame * 2 + 1] = sample;
      phase_ += step;
      if (phase_ >= two_pi)
        phase_ -= two_pi;
    }
  }

  void output_video(uint64_t timestamp) {
    fill_video();
    obs_source_frame frame {};
    frame.data[0] = reinterpret_cast<uint8_t *>(pixels_.data());
    frame.linesize[0] = width_ * sizeof(uint32_t);
    frame.width = width_;
    frame.height = height_;
    frame.timestamp = timestamp;
    frame.format = VIDEO_FORMAT_BGRX;
    obs_source_output_video(source_, &frame);
  }

  void output_audio(uint64_t timestamp) {
    fill_audio();
    obs_source_audio audio {};
    audio.data[0] = reinterpret_cast<const uint8_t *>(samples_.data());
    audio.frames = audio_packet_frames;
    audio.speakers = SPEAKERS_STEREO;
    audio.format = AUDIO_FORMAT_FLOAT;
    audio.samples_per_sec = audio_sample_rate;
    audio.timestamp = timestamp;
    obs_source_output_audio(source_, &audio);
  }

  void run() {
    const uint64_t start = os_gettime_ns();
    const uint64_t video_interval =
        1000000000ULL * fps_denominator_ / fps_numerator_;
    const uint64_t audio_interval =
        1000000000ULL * audio_packet_frames / audio_sample_rate;
    uint64_t next_video = start;
    uint64_t next_audio = start;
    while (!stop_.load()) {
      const uint64_t now = os_gettime_ns();
      if (now >= next_video) {
        output_video(next_video - start);
        next_video += video_interval;
        if (next_video + video_interval * 4 < now)
          next_video = now + video_interval;
      }
      if (now >= next_audio) {
        output_audio(next_audio);
        next_audio += audio_interval;
        if (next_audio + audio_interval * 4 < now)
          next_audio = now + audio_interval;
      }
      const uint64_t wake = std::min(next_video, next_audio);
      if (wake > os_gettime_ns())
        os_sleepto_ns(wake);
    }
  }

  obs_source_t *source_;
  uint32_t width_;
  uint32_t height_;
  uint32_t fps_numerator_;
  uint32_t fps_denominator_;
  uint32_t tone_hz_;
  uint32_t color_;
  std::vector<uint32_t> pixels_;
  std::vector<float> samples_;
  std::atomic<bool> stop_ = false;
  std::thread thread_;
  uint32_t frame_counter_ = 0;
  double phase_ = 0.0;
};

const char *source_name(void *) { return "Live Production Fixture Source"; }

void *source_create(obs_data_t *settings, obs_source_t *source) {
  auto *fixture = new (std::nothrow) FixtureSource(source, settings);
  if (!fixture || !fixture->start()) {
    delete fixture;
    return nullptr;
  }
  return fixture;
}

void source_destroy(void *data) {
  delete static_cast<FixtureSource *>(data);
}

obs_source_info make_source_info() {
  obs_source_info info {};
  info.id = "live_production_fixture_source";
  info.type = OBS_SOURCE_TYPE_INPUT;
  info.output_flags = OBS_SOURCE_ASYNC_VIDEO | OBS_SOURCE_AUDIO |
                      OBS_SOURCE_DO_NOT_DUPLICATE | OBS_SOURCE_CAP_DISABLED;
  info.get_name = source_name;
  info.create = source_create;
  info.destroy = source_destroy;
  return info;
}

obs_source_info source_info = make_source_info();

}

void register_fixture_source() { obs_register_source(&source_info); }

obs_source_t *create_fixture_source(const FixtureSourceCommand &command,
                                    const PublisherConfig &publisher,
                                    std::string &error) {
  obs_data_t *settings = obs_data_create();
  obs_data_set_int(settings, "width", publisher.encoding.width);
  obs_data_set_int(settings, "height", publisher.encoding.height);
  obs_data_set_int(settings, "fpsNumerator", publisher.encoding.fps_numerator);
  obs_data_set_int(settings, "fpsDenominator", publisher.encoding.fps_denominator);
  obs_data_set_int(settings, "toneHz", command.tone_hz);
  obs_data_set_string(settings, "color", command.color.c_str());
  obs_source_t *source = obs_source_create(
      "live_production_fixture_source", command.source_name.c_str(), settings,
      nullptr);
  obs_data_release(settings);
  if (!source) {
    error = "failed to create native fixture source";
    return nullptr;
  }
  obs_source_set_audio_mixers(source, 1u << command.audio_mix_index);
  return source;
}

}
