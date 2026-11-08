#include "fixture_source.hpp"

#include <media-io/audio-io.h>
#include <util/platform.h>

#include <chrono>
#include <array>
#include <cmath>
#include <condition_variable>
#include <cstdlib>
#include <iostream>
#include <mutex>
#include <stdexcept>
#include <unistd.h>

namespace {
struct Signal {
  std::mutex mutex;
  std::condition_variable changed;
  double energy = 0;
  uint64_t samples = 0;
  std::array<double, 2> real {};
  std::array<double, 2> imaginary {};
};

void capture(void *context, size_t, audio_data *audio) {
  auto &signal = *static_cast<Signal *>(context);
  std::lock_guard<std::mutex> lock(signal.mutex);
  const auto *samples = reinterpret_cast<const float *>(audio->data[0]);
  for (uint32_t index = 0; index < audio->frames; ++index) {
    signal.energy += samples[index] * samples[index];
    for (size_t tone = 0; tone < 2; ++tone) {
      const double phase = 6.283185307179586 * (tone == 0 ? 440 : 880) *
                           (signal.samples + index) / 48000.0;
      signal.real[tone] += samples[index] * std::cos(phase);
      signal.imaginary[tone] += samples[index] * std::sin(phase);
    }
  }
  signal.samples += audio->frames;
  signal.changed.notify_all();
}
}

int main(int, char **argv) {
  if (!std::getenv("DISPLAY")) {
    ::execlp("xvfb-run", "xvfb-run", "-a", argv[0], nullptr);
    return 1;
  }
  if (!obs_startup("en-US", nullptr, nullptr)) return 1;
  obs_video_info video {};
  const char *graphics = std::getenv("LIVE_LOCAL_OBS_OPENGL");
  video.graphics_module = graphics ? graphics : "libobs-opengl";
  video.fps_num = 30;
  video.fps_den = 1;
  video.base_width = video.output_width = 64;
  video.base_height = video.output_height = 64;
  video.output_format = VIDEO_FORMAT_NV12;
  video.colorspace = VIDEO_CS_709;
  video.range = VIDEO_RANGE_PARTIAL;
  video.gpu_conversion = true;
  if (obs_reset_video(&video) != OBS_VIDEO_SUCCESS) return 1;
  obs_audio_info info {};
  info.samples_per_sec = 48000;
  info.speakers = SPEAKERS_STEREO;
  if (!obs_reset_audio(&info)) return 1;
  live_production::register_fixture_source();
  live_production::PublisherConfig config;
  config.encoding.width = 64;
  config.encoding.height = 64;
  config.encoding.fps_numerator = 30;
  config.encoding.fps_denominator = 1;
  live_production::FixtureSourceCommand command;
  command.source_name = "Audio root regression";
  command.color = "red";
  command.tone_hz = 440;
  command.audio_mix_index = 0;
  std::string error;
  obs_source_t *source = live_production::create_fixture_source(command, config, error);
  if (!source) return 1;
  command.source_name = "Second audio root regression";
  command.tone_hz = 880;
  command.audio_mix_index = 1;
  obs_source_t *second = live_production::create_fixture_source(command, config, error);
  if (!second) return 1;
  std::array<Signal, 2> signals;
  const bool connected = audio_output_connect(obs_get_audio(), 0, nullptr, capture, &signals[0]);
  const bool second_connected = audio_output_connect(obs_get_audio(), 1, nullptr, capture, &signals[1]);
  bool complete = true;
  for (auto &signal : signals) {
    std::unique_lock<std::mutex> lock(signal.mutex);
    const bool observed = signal.changed.wait_for(lock, std::chrono::seconds(4), [&] {
      return signal.samples >= 48000;
    });
    complete = complete && observed;
  }
  if (connected) audio_output_disconnect(obs_get_audio(), 0, capture, &signals[0]);
  if (second_connected) audio_output_disconnect(obs_get_audio(), 1, capture, &signals[1]);
  live_production::release_fixture_source(source);
  live_production::release_fixture_source(second);
  obs_shutdown();
  bool valid = complete && connected && second_connected;
  for (size_t mix = 0; mix < signals.size(); ++mix) {
    const auto &signal = signals[mix];
    const double rms = signal.samples == 0 ? 0 : std::sqrt(signal.energy / signal.samples);
    const auto amplitude = [&](size_t tone) {
      return signal.samples == 0 ? 0 : 2 * std::hypot(signal.real[tone], signal.imaginary[tone]) / signal.samples;
    };
    std::cout << "fixture_mix=" << mix << " rms=" << rms << " tone_amplitude="
              << amplitude(mix) << " other_tone=" << amplitude(1 - mix) << '\n';
    valid = valid && rms >= 0.001 && amplitude(mix) >= 0.05 && amplitude(1 - mix) < 0.001;
  }
  return valid ? 0 : 1;
}
