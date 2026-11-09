#include "live_srt_output.hpp"

#include "live_muxer.hpp"
#include "live_queue.hpp"
#include "live_srt_transport.hpp"

#include <obs-module.h>

#include <atomic>
#include <chrono>
#include <cstring>
#include <memory>
#include <mutex>
#include <string>
#include <thread>

namespace live_production {

namespace {

class LiveOutput {
 public:
  LiveOutput(obs_data_t *settings, obs_output_t *output)
      : output_(output), srt_(read_srt(settings)), mux_settings_(read_mux(settings)),
        valid_(live_srt_settings_valid(srt_, mux_settings_)) {
    proc_handler_t *handler = obs_output_get_proc_handler(output_);
    proc_handler_add(handler, "void live_production_freeze(out bool success, out int epoch)",
                     freeze_proc, this);
    proc_handler_add(handler, "void live_production_resume(int epoch, out bool success)",
                     resume_proc, this);
  }

  ~LiveOutput() { stop(); }

  bool start() {
    if (!valid_ || worker_.joinable() || !obs_output_can_begin_data_capture(output_, 0) ||
        !obs_output_initialize_encoders(output_, 0))
      return false;
    video_ = obs_output_get_video_encoder(output_);
    audio_ = obs_output_get_audio_encoder(output_, 0);
    obs_audio_info audio_info{};
    if (!video_ || !audio_ ||
        std::strcmp(obs_encoder_get_codec(video_), "h264") != 0 ||
        std::strcmp(obs_encoder_get_codec(audio_), "aac") != 0 ||
        obs_encoder_get_width(video_) != mux_settings_.width ||
        obs_encoder_get_height(video_) != mux_settings_.height ||
        obs_encoder_get_sample_rate(audio_) != 48000 ||
        !obs_get_audio_info(&audio_info) || audio_info.speakers != SPEAKERS_STEREO)
      return false;
    stopping_.store(false);
    signaled_.store(false);
    last_bytes_.store(0);
    auto session = std::make_shared<Session>(srt_, epoch_seed_.load());
    {
      std::lock_guard<std::mutex> lock(session_mutex_);
      session_ = session;
    }
    try {
      worker_ = std::thread([this, session] { run(session); });
    } catch (...) {
      session->queue.stop();
      session->transport.interrupt();
      std::lock_guard<std::mutex> lock(session_mutex_);
      session_.reset();
      return false;
    }
    return true;
  }

  void stop() {
    stopping_.store(true);
    const auto session = current_session();
    if (session) {
      int epoch = 0;
      (void)session->queue.freeze(epoch, std::chrono::milliseconds(500));
      session->queue.stop();
      session->transport.interrupt();
    }
    if (worker_.joinable() && worker_.get_id() != std::this_thread::get_id())
      worker_.join();
    end_capture();
    std::lock_guard<std::mutex> lock(session_mutex_);
    session_.reset();
  }

  void packet(encoder_packet *packet) {
    if (!packet) {
      if (const auto session = current_session()) session->queue.fail();
      return;
    }
    if (const auto session = current_session()) (void)session->queue.push(packet);
  }

  uint64_t total_bytes() const {
    const auto session = current_session();
    return session ? session->transport.sent_bytes() : last_bytes_.load();
  }

  bool freeze(int &epoch) {
    const auto session = current_session();
    if (!session || !session->queue.freeze(epoch, std::chrono::milliseconds(500)))
      return false;
    epoch_seed_.store(epoch);
    return true;
  }

  bool resume(int epoch) {
    const auto session = current_session();
    return session && session->queue.resume(epoch);
  }

 private:
  static LiveSrtSettings read_srt(obs_data_t *settings) {
    LiveSrtSettings value;
    value.host = obs_data_get_string(settings, "host");
    value.port = static_cast<int>(obs_data_get_int(settings, "port"));
    value.passphrase = obs_data_get_string(settings, "passphrase");
    value.stream_id = obs_data_get_string(settings, "streamId");
    value.latency_ms = static_cast<int>(obs_data_get_int(settings, "bufferMs"));
    return value;
  }

  static LiveMuxSettings read_mux(obs_data_t *settings) {
    LiveMuxSettings value;
    value.width = static_cast<uint32_t>(obs_data_get_int(settings, "width"));
    value.height = static_cast<uint32_t>(obs_data_get_int(settings, "height"));
    value.fps_num = static_cast<uint32_t>(obs_data_get_int(settings, "fpsNumerator"));
    value.fps_den = static_cast<uint32_t>(obs_data_get_int(settings, "fpsDenominator"));
    return value;
  }

  struct Session {
    Session(const LiveSrtSettings &settings, int epoch)
        : queue(epoch), transport(settings) {}
    LiveQueue queue;
    LiveSrtTransport transport;
  };

  std::shared_ptr<Session> current_session() const {
    std::lock_guard<std::mutex> lock(session_mutex_);
    return session_;
  }

  void run(const std::shared_ptr<Session> &session) {
    struct TransportGuard {
      LiveSrtTransport &transport;
      ~TransportGuard() { transport.finish(); }
    } transport_guard{session->transport};
    // PR_SET_PDEATHSIG follows the spawning thread: drain and reap the helper
    // here, before that worker exits, rather than in the later Session teardown.
    LiveMuxer muxer(mux_settings_, video_, audio_, session->queue, session->transport);
    if (!session->transport.connect() || stopping_.load()) {
      if (!stopping_.load()) fail_from_worker(session);
      return;
    }
    bool capture_started = false;
    {
      std::lock_guard<std::mutex> lock(lifecycle_mutex_);
      if (stopping_.load()) return;
      capture_ended_.store(false);
      capture_started = obs_output_begin_data_capture(output_, 0);
      if (!capture_started) capture_ended_.store(true);
    }
    if (!capture_started) {
      fail_from_worker(session);
      return;
    }
    bool muxed = true;
    bool opened = false;
    while (const auto work = session->queue.pop()) {
      if (!session->queue.begin_processing(work->epoch)) continue;
      struct ProcessingGuard {
        LiveQueue &queue;
        ~ProcessingGuard() { queue.end_processing(); }
      } processing{session->queue};
      bool processed = true;
      if (!opened) {
        opened = muxer.open(work->epoch);
        processed = opened;
      }
      if (processed) processed = muxer.write(*work);
      if (!processed) {
        muxed = false;
        break;
      }
    }
    last_bytes_.store(session->transport.sent_bytes());
    if ((!muxed || session->queue.failed()) && !stopping_.load())
      fail_from_worker(session);
  }

  void fail_from_worker(const std::shared_ptr<Session> &session) {
    session->queue.stop();
    session->transport.interrupt();
    capture_ended_.store(true);
    if (!signaled_.exchange(true))
      obs_output_signal_stop(output_, OBS_OUTPUT_ERROR);
  }

  void end_capture() {
    std::lock_guard<std::mutex> lock(lifecycle_mutex_);
    if (!capture_ended_.exchange(true)) obs_output_end_data_capture(output_);
  }

  static void freeze_proc(void *data, calldata_t *params) {
    int epoch = 0;
    const bool success = static_cast<LiveOutput *>(data)->freeze(epoch);
    calldata_set_bool(params, "success", success);
    calldata_set_int(params, "epoch", epoch);
  }

  static void resume_proc(void *data, calldata_t *params) {
    const int epoch = static_cast<int>(calldata_int(params, "epoch"));
    calldata_set_bool(params, "success", static_cast<LiveOutput *>(data)->resume(epoch));
  }

  obs_output_t *output_;
  LiveSrtSettings srt_;
  LiveMuxSettings mux_settings_;
  obs_encoder_t *video_ = nullptr;
  obs_encoder_t *audio_ = nullptr;
  bool valid_ = false;
  mutable std::mutex session_mutex_;
  std::shared_ptr<Session> session_;
  std::thread worker_;
  std::mutex lifecycle_mutex_;
  std::atomic<bool> stopping_{false};
  std::atomic<bool> capture_ended_{true};
  std::atomic<bool> signaled_{false};
  std::atomic<int> epoch_seed_{0};
  std::atomic<uint64_t> last_bytes_{0};
};

const char *name(void *) { return "Live Production MPEG-TS SRT"; }

void *create(obs_data_t *settings, obs_output_t *output) {
  try {
    return new LiveOutput(settings, output);
  } catch (...) {
    return nullptr;
  }
}

void destroy(void *data) { delete static_cast<LiveOutput *>(data); }
bool start(void *data) {
  try {
    return static_cast<LiveOutput *>(data)->start();
  } catch (...) {
    return false;
  }
}
void stop(void *data, uint64_t) { static_cast<LiveOutput *>(data)->stop(); }
void packet(void *data, encoder_packet *value) {
  try {
    static_cast<LiveOutput *>(data)->packet(value);
  } catch (...) {
    static_cast<LiveOutput *>(data)->packet(nullptr);
  }
}
uint64_t bytes(void *data) { return static_cast<LiveOutput *>(data)->total_bytes(); }

obs_output_info make_output_info() {
  obs_output_info info{};
  info.id = "live_production_mpegts_srt";
  info.flags = OBS_OUTPUT_AV | OBS_OUTPUT_ENCODED;
  info.get_name = name;
  info.create = create;
  info.destroy = destroy;
  info.start = start;
  info.stop = stop;
  info.encoded_packet = packet;
  info.get_total_bytes = bytes;
  info.encoded_video_codecs = "h264";
  info.encoded_audio_codecs = "aac";
  return info;
}

obs_output_info output_info = make_output_info();

}

void register_live_srt_output() { obs_register_output(&output_info); }

}
