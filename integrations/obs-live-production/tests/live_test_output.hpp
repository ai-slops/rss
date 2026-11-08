#pragma once

#include <atomic>
#include <string>

#include <obs.h>

class LiveTestReceiver;

class LiveTestOutput {
 public:
  LiveTestOutput(obs_source_t *source, LiveTestReceiver &receiver,
                 const std::string &name,
                 const char *passphrase = "fixtureCredential42",
                 uint32_t fps_num = 30, uint32_t fps_den = 1);
  ~LiveTestOutput();
  bool start();
  void stop();
  int freeze();
  bool resume(int epoch);
  int starts() const { return starts_.load(); }
  int stops() const { return stops_.load(); }
  int stop_code() const { return stop_code_.load(); }

 private:
  static void started(void *data, calldata_t *params);
  static void stopped(void *data, calldata_t *params);
  obs_view_t *view_ = nullptr;
  video_t *view_video_ = nullptr;
  obs_encoder_t *video_encoder_ = nullptr;
  obs_encoder_t *audio_encoder_ = nullptr;
  obs_output_t *output_ = nullptr;
  std::atomic<int> starts_{0};
  std::atomic<int> stops_{0};
  std::atomic<int> stop_code_{0};
};
