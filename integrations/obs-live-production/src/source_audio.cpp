#include "source_audio.hpp"
#include <algorithm>
#include <cstring>

namespace live_production {
SourceAudio::SourceAudio(obs_source_t *source) : source_(source) {}
SourceAudio::~SourceAudio() {
  if (active_) {
    obs_source_remove_audio_capture_callback(source_, capture, this);
    obs_source_dec_active(source_);
  }
  if (output_) audio_output_close(output_);
}
bool SourceAudio::initialize() {
  audio_output_info info {};
  info.name = obs_source_get_name(source_);
  info.samples_per_sec = 48000;
  info.format = AUDIO_FORMAT_FLOAT_PLANAR;
  info.speakers = SPEAKERS_STEREO;
  info.input_callback = pull;
  info.input_param = this;
  if (audio_output_open(&output_, &info) != AUDIO_OUTPUT_SUCCESS) return false;
  obs_source_add_audio_capture_callback(source_, capture, this);
  obs_source_inc_active(source_);
  active_ = true;
  return true;
}
void SourceAudio::capture(void *context, obs_source_t *, const audio_data *data, bool muted) {
  auto &self = *static_cast<SourceAudio *>(context);
  if (!data || !data->frames || data->frames > 48000 || !data->data[0]) return;
  Packet packet {data->frames, {}};
  for (size_t channel = 0; channel < 2; ++channel) {
    const auto *samples = reinterpret_cast<const float *>(data->data[channel]);
    if (samples && !muted) packet.samples[channel].assign(samples, samples + data->frames);
    else packet.samples[channel].assign(data->frames, 0.0f);
  }
  std::lock_guard<std::mutex> lock(self.mutex_);
  // Bound queued capture to about one second. Never block OBS's audio thread.
  if (self.packets_.size() >= 64) { self.packets_.pop_front(); self.packet_offset_ = 0; }
  self.packets_.push_back(std::move(packet));
}
bool SourceAudio::pull(void *context, uint64_t start, uint64_t, uint64_t *timestamp,
                       uint32_t, audio_output_data *mixes) {
  auto &self = *static_cast<SourceAudio *>(context);
  *timestamp = start;
  for (size_t channel = 0; channel < 2; ++channel)
    std::fill_n(mixes[0].data[channel], AUDIO_OUTPUT_FRAMES, 0.0f);
  std::lock_guard<std::mutex> lock(self.mutex_);
  // Source capture and independent mixer timestamps have different origins.
  // Consume ordered samples on this output clock instead of comparing epochs.
  size_t written = 0;
  while (written < AUDIO_OUTPUT_FRAMES && !self.packets_.empty()) {
    const auto &packet = self.packets_.front();
    const size_t count = std::min(size_t(packet.frames) - self.packet_offset_, size_t(AUDIO_OUTPUT_FRAMES) - written);
    for (size_t channel = 0; channel < 2; ++channel)
      std::copy_n(packet.samples[channel].data() + self.packet_offset_, count, mixes[0].data[channel] + written);
    written += count;
    self.packet_offset_ += count;
    if (self.packet_offset_ == packet.frames) { self.packets_.pop_front(); self.packet_offset_ = 0; }
  }
  return true;
}
}
