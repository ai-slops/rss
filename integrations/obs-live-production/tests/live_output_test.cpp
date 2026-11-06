#include "config.hpp"
#include "fixture_source.hpp"
#include "live_srt_output.hpp"
#include "live_test_decode.hpp"
#include "live_test_output.hpp"
#include "live_test_receiver.hpp"
#include "publisher.hpp"

#include <obs-module.h>
#include <QCoreApplication>

extern "C" {
#include <libavutil/avutil.h>
}

#include <chrono>
#include <atomic>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <stdexcept>
#include <string>
#include <thread>
#include <unistd.h>

namespace {

using namespace std::chrono_literals;

void require(bool value, const char *message) {
  if (!value) throw std::runtime_error(message);
}

void load_module(const std::string &root, const char *name) {
  obs_module_t *module = nullptr;
  std::string binary = root + "/usr/lib/obs-plugins/" + name + ".so";
  std::string data = root + "/usr/share/obs/obs-plugins/" + name;
  if (root.empty()) {
    struct Found {
      std::string name;
      std::string binary;
      std::string data;
    } found{name, {}, {}};
    obs_find_modules2([](void *context, const obs_module_info2 *info) {
      auto &result = *static_cast<Found *>(context);
      if (info->name && result.name == info->name) {
        result.binary = info->bin_path;
        result.data = info->data_path;
      }
    }, &found);
    require(!found.binary.empty(), "installed OBS encoder module not found");
    binary = std::move(found.binary);
    data = std::move(found.data);
  }
  require(obs_open_module(&module, binary.c_str(), data.c_str()) == MODULE_SUCCESS,
          "OBS module open failed");
  require(obs_init_module(module), "OBS module init failed");
}

bool wait_bytes(const LiveTestReceiver &receiver, uint64_t target,
                std::chrono::milliseconds timeout) {
  const auto deadline = std::chrono::steady_clock::now() + timeout;
  while (std::chrono::steady_clock::now() < deadline) {
    if (receiver.bytes() >= target) return true;
    std::this_thread::sleep_for(20ms);
  }
  return false;
}

bool wait_stops(const LiveTestOutput &output, int target,
                std::chrono::milliseconds timeout) {
  const auto deadline = std::chrono::steady_clock::now() + timeout;
  while (std::chrono::steady_clock::now() < deadline) {
    if (output.stops() >= target) return true;
    std::this_thread::sleep_for(20ms);
  }
  return false;
}

}

int main(int argc, char **argv) {
  if (!std::getenv("DISPLAY")) {
    ::execlp("xvfb-run", "xvfb-run", "-a", argv[0], argc > 1 ? argv[1] : nullptr, nullptr);
    return 1;
  }
  try {
    const bool ntsc = argc == 2 && std::string(argv[1]) == "--ntsc";
    const uint32_t fps_num = ntsc ? 30000 : 30;
    const uint32_t fps_den = ntsc ? 1001 : 1;
    int qt_argc = 1;
    char application_name[] = "live-output-test";
    char *qt_argv[] = {application_name, nullptr};
    QCoreApplication application(qt_argc, qt_argv);
    require(obs_startup("en-US", nullptr, nullptr), "OBS startup failed");
    const char *sdk_root = std::getenv("LIVE_OBS_SDK_ROOT");
    const std::string sdk = sdk_root ? sdk_root : "";
    obs_video_info video{};
    const char *graphics = std::getenv("LIVE_LOCAL_OBS_OPENGL");
    video.graphics_module = graphics ? graphics : "libobs-opengl";
    video.fps_num = fps_num;
    video.fps_den = fps_den;
    video.base_width = video.output_width = 64;
    video.base_height = video.output_height = 64;
    video.output_format = VIDEO_FORMAT_NV12;
    video.colorspace = VIDEO_CS_709;
    video.range = VIDEO_RANGE_PARTIAL;
    video.gpu_conversion = true;
    require(obs_reset_video(&video) == OBS_VIDEO_SUCCESS, "video reset failed");
    obs_audio_info audio{48000, SPEAKERS_STEREO};
    require(obs_reset_audio(&audio), "audio reset failed");
    load_module(sdk, "obs-x264");
    load_module(sdk, "obs-ffmpeg");
    live_production::register_fixture_source();
    live_production::register_live_srt_output();
    live_production::PublisherConfig config;
    config.encoding.width = config.encoding.height = 64;
    config.encoding.fps_numerator = fps_num;
    config.encoding.fps_denominator = fps_den;
    config.encoding.video_bitrate_kbps = 500;
    config.encoding.audio_sample_rate = 48000;
    live_production::FixtureSourceCommand command;
    command.source_name = "Live output source";
    command.color = "red";
    command.tone_hz = 440;
    command.audio_mix_index = 0;
    std::string error;
    obs_source_t *source = live_production::create_fixture_source(command, config, error);
    require(source, "fixture source failed");
    LiveTestReceiver receiver_a;
    LiveTestReceiver receiver_b;
    LiveTestReceiver receiver_bad;
    LiveTestReceiver receiver_publisher;
    require(receiver_a.start() && receiver_b.start() && receiver_bad.start() &&
                receiver_publisher.start(),
            "receiver start failed");
    uint64_t frozen_bytes = 0;
    uint64_t resumed_bytes = 0;
    uint64_t other_growth = 0;
    int freeze_epoch = 0;
    int success_starts = 0;
    int success_stops = 0;
    int error_stops = 0;
    {
      LiveTestOutput first(source, receiver_a, "live-a", "fixtureCredential42", fps_num,
                           fps_den);
      LiveTestOutput second(source, receiver_b, "live-b", "fixtureCredential42", fps_num,
                            fps_den);
      require(first.start() && second.start(), "output start failed");
      require(wait_bytes(receiver_a, 30000, 8s) && wait_bytes(receiver_b, 30000, 8s),
              "initial SRT media missing");
      freeze_epoch = first.freeze();
      require(freeze_epoch > 0, "freeze failed");
      std::this_thread::sleep_for(400ms);
      frozen_bytes = receiver_a.bytes();
      const uint64_t other_bytes = receiver_b.bytes();
      std::this_thread::sleep_for(700ms);
      require(receiver_a.bytes() == frozen_bytes, "frozen output advanced");
      require(receiver_b.bytes() > other_bytes, "other output stalled");
      other_growth = receiver_b.bytes() - other_bytes;
      require(!first.resume(freeze_epoch + 1), "stale resume accepted");
      require(first.resume(freeze_epoch) && first.resume(freeze_epoch),
              "matching resume failed");
      require(wait_bytes(receiver_a, frozen_bytes + 20000, 5s), "resumed output missing");
      resumed_bytes = receiver_a.bytes();
      int stop_epoch = first.freeze();
      require(stop_epoch > freeze_epoch, "second freeze epoch did not advance");
      const auto stop_started = std::chrono::steady_clock::now();
      first.stop();
      require(std::chrono::steady_clock::now() - stop_started < 2s,
              "frozen output stop exceeded deadline");
      require(receiver_a.wait_completed(1, 2s), "first SRT session did not drain");
      const uint64_t restart_base = receiver_a.bytes();
      require(first.start(), "same output restart rejected");
      require(wait_bytes(receiver_a, restart_base + 20000, 5s),
              "same output restart produced no media");
      first.stop();
      second.stop();
      require(receiver_a.wait_completed(2, 2s) && receiver_b.wait_completed(1, 2s),
              "healthy SRT sessions did not drain");
      require(first.starts() == 2 && first.stops() == 2 &&
                  first.stop_code() == OBS_OUTPUT_SUCCESS,
              "restart callback counts were incorrect");
      LiveTestOutput bad(source, receiver_bad, "live-bad", "wrongCredential42");
      require(bad.start(), "bad-auth output did not enter starting state");
      require(wait_stops(bad, 1, 5s), "bad-auth output did not fail");
      require(bad.starts() == 0 && bad.stops() == 1 &&
                  bad.stop_code() != OBS_OUTPUT_SUCCESS,
              "bad-auth output signalled success or duplicate Stop");
      success_starts = first.starts();
      success_stops = first.stops();
      error_stops = bad.stops();
    }
    config.obs.source_name = command.source_name;
    config.obs.audio_mix_index = 0;
    config.target.host = "127.0.0.1";
    config.target.port = receiver_publisher.port();
    config.target.passphrase = "fixtureCredential42";
    config.target.stream_id = "publisher-backed";
    config.target.buffer_ms = 120;
    {
      live_production::Publisher publisher(config, "component", [] {});
      require(publisher.initialize(error), "Publisher initialize failed");
      require(publisher.start(error), "Publisher start failed");
      require(wait_bytes(receiver_publisher, 30000, 8s), "Publisher SRT media missing");
      require(publisher.freeze_transport(), "Publisher freeze proc failed");
      require(publisher.resume_transport(), "Publisher resume proc failed");
      publisher.stop();
    }
    require(receiver_publisher.wait_completed(1, 2s), "Publisher SRT session did not drain");
    receiver_a.stop();
    receiver_b.stop();
    receiver_bad.stop();
    receiver_publisher.stop();
    const auto captures_a = receiver_a.captures();
    require(captures_a.size() == 2, "restart did not produce two separate SRT sessions");
    const LiveDecodeResult decoded_a = decode_live_mpegts(captures_a.front());
    require(decode_live_mpegts(captures_a.back()).valid, "restarted SRT session failed strict decode");
    const LiveDecodeResult decoded_b = decode_live_mpegts(receiver_b.data());
    const LiveDecodeResult decoded_publisher = decode_live_mpegts(receiver_publisher.data());
    require(decoded_a.valid && decoded_b.valid && decoded_publisher.valid,
            "strict MPEG-TS streams missing");
    live_production::release_fixture_source(source);
    obs_shutdown();
    std::cout << "live output component passed epoch=" << freeze_epoch
              << " frozen_bytes=" << frozen_bytes
              << " resumed_growth=" << (resumed_bytes - frozen_bytes)
              << " other_growth=" << other_growth
              << " packets_a=" << receiver_a.packets()
              << " packets_b=" << receiver_b.packets()
              << " success_starts=" << success_starts
              << " success_stops=" << success_stops
              << " error_stops=" << error_stops
              << " video_s=" << decoded_a.video_seconds
              << " audio_s=" << decoded_a.audio_seconds
              << " ntsc=" << ntsc
              << " obs=" << obs_get_version_string()
              << " ffmpeg=" << av_version_info()
              << " srt=" << srt_getversion() << '\n';
  } catch (const std::exception &failure) {
    std::cerr << failure.what() << '\n';
    return 1;
  }
}
