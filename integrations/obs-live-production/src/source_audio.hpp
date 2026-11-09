#pragma once
#include <obs.h>
#include <array>
#include <deque>
#include <mutex>
#include <vector>

namespace live_production {
class SourceAudio {
public:
  explicit SourceAudio(obs_source_t *source);
  ~SourceAudio();
  bool initialize();
  audio_t *output() const { return output_; }
private:
  struct Packet { uint32_t frames; std::array<std::vector<float>, 2> samples; };
  static void capture(void *, obs_source_t *, const audio_data *, bool);
  static bool pull(void *, uint64_t, uint64_t, uint64_t *, uint32_t, audio_output_data *);
  obs_source_t *source_;
  audio_t *output_ = nullptr;
  bool active_ = false;
  std::mutex mutex_;
  std::deque<Packet> packets_;
  size_t packet_offset_ = 0;
};
}
