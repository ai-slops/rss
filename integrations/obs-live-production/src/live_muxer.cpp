#include "live_muxer.hpp"

#include <obs.h>

extern "C" {
#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include <libavutil/channel_layout.h>
#include <libavutil/mathematics.h>
}

#include <cstring>

namespace live_production {

int64_t rescale_obs_timestamp(int64_t value, AVRational obs_time_base,
                              AVRational stream_time_base) {
  if (value == AV_NOPTS_VALUE) return value;
  if (obs_time_base.num <= 0 || obs_time_base.den <= 0 ||
      stream_time_base.num <= 0 || stream_time_base.den <= 0)
    return AV_NOPTS_VALUE;
  value /= obs_time_base.num;
  return av_rescale_q_rnd(value, obs_time_base, stream_time_base,
                          static_cast<AVRounding>(AV_ROUND_NEAR_INF |
                                                  AV_ROUND_PASS_MINMAX));
}

LiveMuxer::LiveMuxer(LiveMuxSettings settings, obs_encoder *video,
                     obs_encoder *audio, LiveQueue &queue,
                     LiveSrtTransport &transport)
    : settings_(settings), video_(video), audio_(audio), queue_(queue),
      transport_(transport) {}

LiveMuxer::~LiveMuxer() { close(); }

bool LiveMuxer::open(int epoch) {
  active_epoch_ = epoch;
  if (avformat_alloc_output_context2(&format_, nullptr, "mpegts", nullptr) < 0)
    return false;
  uint8_t *buffer = static_cast<uint8_t *>(av_malloc(1316));
  if (!buffer) return false;
  avio_ = avio_alloc_context(buffer, 1316, 1, this, nullptr, write_packet, nullptr);
  if (!avio_) {
    av_free(buffer);
    return false;
  }
  format_->pb = avio_;
  format_->flags |= AVFMT_FLAG_CUSTOM_IO | AVFMT_FLAG_AUTO_BSF;
  if (!add_streams()) return false;
  return avformat_write_header(format_, nullptr) >= 0;
}

bool LiveMuxer::add_streams() {
  video_stream_ = avformat_new_stream(format_, nullptr);
  audio_stream_ = avformat_new_stream(format_, nullptr);
  if (!video_stream_ || !audio_stream_) return false;
  video_stream_->time_base = AVRational{static_cast<int>(settings_.fps_den),
                                        static_cast<int>(settings_.fps_num)};
  AVCodecParameters *video = video_stream_->codecpar;
  video->codec_type = AVMEDIA_TYPE_VIDEO;
  video->codec_id = AV_CODEC_ID_H264;
  video->width = static_cast<int>(settings_.width);
  video->height = static_cast<int>(settings_.height);
  audio_stream_->time_base = AVRational{1, 48000};
  AVCodecParameters *audio = audio_stream_->codecpar;
  audio->codec_type = AVMEDIA_TYPE_AUDIO;
  audio->codec_id = AV_CODEC_ID_AAC;
  audio->sample_rate = 48000;
  av_channel_layout_default(&audio->ch_layout, 2);
  audio->frame_size = static_cast<int>(obs_encoder_get_frame_size(audio_));
  return add_extra_data(video_stream_, video_) &&
         add_extra_data(audio_stream_, audio_);
}

bool LiveMuxer::add_extra_data(AVStream *stream, obs_encoder *encoder) {
  uint8_t *data = nullptr;
  size_t size = 0;
  if (!obs_encoder_get_extra_data(encoder, &data, &size) || !data || size == 0)
    return false;
  stream->codecpar->extradata = static_cast<uint8_t *>(
      av_mallocz(size + AV_INPUT_BUFFER_PADDING_SIZE));
  if (!stream->codecpar->extradata) return false;
  std::memcpy(stream->codecpar->extradata, data, size);
  stream->codecpar->extradata_size = static_cast<int>(size);
  return true;
}

bool LiveMuxer::write(const LiveWork &work) {
  active_epoch_ = work.epoch;
  return write_packet_value(work.packet.get());
}

bool LiveMuxer::write_packet_value(const encoder_packet &packet) {
  AVStream *stream = packet.type == OBS_ENCODER_VIDEO ? video_stream_ : audio_stream_;
  if (!stream) return false;
  AVPacket *value = av_packet_alloc();
  if (!value) return false;
  value->data = packet.data;
  value->size = static_cast<int>(packet.size);
  value->stream_index = stream->index;
  const AVRational source{packet.timebase_num, packet.timebase_den};
  value->pts = rescale_obs_timestamp(packet.pts, source, stream->time_base);
  value->dts = rescale_obs_timestamp(packet.dts, source, stream->time_base);
  value->flags = packet.keyframe ? AV_PKT_FLAG_KEY : 0;
  int result = av_interleaved_write_frame(format_, value);
  av_packet_free(&value);
  if (result >= 0) result = av_interleaved_write_frame(format_, nullptr);
  if (result >= 0) result = av_write_frame(format_, nullptr);
  avio_flush(avio_);
  return result >= 0 && avio_->error >= 0;
}

#if LIBAVFORMAT_VERSION_MAJOR < 61
int LiveMuxer::write_packet(void *opaque, uint8_t *data, int size) {
#else
int LiveMuxer::write_packet(void *opaque, const uint8_t *data, int size) {
#endif
  auto *self = static_cast<LiveMuxer *>(opaque);
  if (!self->queue_.begin_send(self->active_epoch_))
    return self->queue_.discard_send() ? size : AVERROR_EXIT;
  const bool sent = self->transport_.send(data, static_cast<size_t>(size));
  self->queue_.end_send();
  if (!sent) return AVERROR(EIO);
  self->total_bytes_ += static_cast<uint64_t>(size);
  return size;
}

void LiveMuxer::close() {
  if (avio_) {
    av_freep(&avio_->buffer);
    avio_context_free(&avio_);
  }
  if (format_) avformat_free_context(format_);
  format_ = nullptr;
  video_stream_ = nullptr;
  audio_stream_ = nullptr;
}

}
