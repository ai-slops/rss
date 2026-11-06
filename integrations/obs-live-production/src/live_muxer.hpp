#pragma once

#include "live_queue.hpp"
#include "live_srt_transport.hpp"

#include <cstdint>

extern "C" {
#include <libavformat/version_major.h>
#include <libavutil/rational.h>
}

struct AVFormatContext;
struct AVIOContext;
struct AVStream;
struct obs_encoder;

namespace live_production {

int64_t rescale_obs_timestamp(int64_t value, AVRational obs_time_base,
                              AVRational stream_time_base);

class LiveMuxer {
 public:
  LiveMuxer(LiveMuxSettings settings, obs_encoder *video, obs_encoder *audio,
            LiveQueue &queue, LiveSrtTransport &transport);
  ~LiveMuxer();
  bool open(int epoch);
  bool write(const LiveWork &work);
  void close();
  uint64_t total_bytes() const { return total_bytes_; }

 private:
#if LIBAVFORMAT_VERSION_MAJOR < 61
  static int write_packet(void *opaque, uint8_t *data, int size);
#else
  static int write_packet(void *opaque, const uint8_t *data, int size);
#endif
  bool add_streams();
  bool add_extra_data(AVStream *stream, obs_encoder *encoder);
  bool write_packet_value(const encoder_packet &packet);
  LiveMuxSettings settings_;
  obs_encoder *video_;
  obs_encoder *audio_;
  LiveQueue &queue_;
  LiveSrtTransport &transport_;
  AVFormatContext *format_ = nullptr;
  AVIOContext *avio_ = nullptr;
  AVStream *video_stream_ = nullptr;
  AVStream *audio_stream_ = nullptr;
  uint64_t total_bytes_ = 0;
  int active_epoch_ = 0;
};

}
