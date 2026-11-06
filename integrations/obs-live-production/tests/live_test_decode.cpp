#include "live_test_decode.hpp"

extern "C" {
#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
}

#include <filesystem>
#include <fstream>
#include <cmath>
#include <string>
#include <unistd.h>
#include <iostream>
#include <cstdlib>

namespace {

struct AudioMetrics {
  double energy = 0;
  double real = 0;
  double imaginary = 0;
  uint64_t samples = 0;
};

bool audio_frame(const AVFrame *frame, AudioMetrics &metrics, AVRational time_base) {
  if (frame->format != AV_SAMPLE_FMT_FLTP || frame->ch_layout.nb_channels != 2)
    return false;
  const auto *left = reinterpret_cast<const float *>(frame->extended_data[0]);
  const auto *right = reinterpret_cast<const float *>(frame->extended_data[1]);
  for (int index = 0; index < frame->nb_samples; ++index) {
    const double sample = (left[index] + right[index]) * 0.5;
    const double seconds = frame->best_effort_timestamp == AV_NOPTS_VALUE
                               ? metrics.samples / 48000.0
                               : frame->best_effort_timestamp * av_q2d(time_base) + index / 48000.0;
    const double phase = 6.283185307179586 * 440.0 * seconds;
    metrics.energy += sample * sample;
    metrics.real += sample * std::cos(phase);
    metrics.imaginary += sample * std::sin(phase);
    ++metrics.samples;
  }
  return true;
}

struct StreamResult {
  int frames = 0;
  double seconds = 0;
};

StreamResult decode_stream(const std::filesystem::path &path, AVMediaType type,
                           AudioMetrics *metrics) {
  AVFormatContext *format = avformat_alloc_context();
  if (!format) return {};
  format->error_recognition = AV_EF_EXPLODE;
  if (avformat_open_input(&format, path.c_str(), nullptr, nullptr) < 0 ||
      avformat_find_stream_info(format, nullptr) < 0) {
    avformat_close_input(&format);
    return {};
  }
  const int index = av_find_best_stream(format, type, -1, -1, nullptr, 0);
  if (index < 0) {
    avformat_close_input(&format);
    return {};
  }
  const AVCodec *codec = avcodec_find_decoder(format->streams[index]->codecpar->codec_id);
  AVCodecContext *context = codec ? avcodec_alloc_context3(codec) : nullptr;
  if (!context ||
      avcodec_parameters_to_context(context, format->streams[index]->codecpar) < 0) {
    avcodec_free_context(&context);
    avformat_close_input(&format);
    return {};
  }
  context->err_recognition = AV_EF_EXPLODE;
  context->pkt_timebase = format->streams[index]->time_base;
  if (avcodec_open2(context, codec, nullptr) < 0) {
    avcodec_free_context(&context);
    avformat_close_input(&format);
    return {};
  }
  AVPacket *packet = av_packet_alloc();
  AVFrame *frame = av_frame_alloc();
  const AVRational time_base = format->streams[index]->time_base;
  int frames = 0;
  int64_t first = AV_NOPTS_VALUE;
  int64_t last = AV_NOPTS_VALUE;
  const auto consume = [&] {
    int result = 0;
    while ((result = avcodec_receive_frame(context, frame)) >= 0) {
      if (metrics && !audio_frame(frame, *metrics, time_base)) return AVERROR_INVALIDDATA;
      const int64_t timestamp = frame->best_effort_timestamp;
      if (timestamp != AV_NOPTS_VALUE) {
        if (first == AV_NOPTS_VALUE) first = timestamp;
        last = timestamp + frame->duration;
      }
      ++frames;
    }
    return result;
  };
  int demux_result = 0;
  while ((demux_result = av_read_frame(format, packet)) >= 0) {
    if (packet->flags & AV_PKT_FLAG_CORRUPT) {
      frames = -1;
      break;
    }
    if (packet->stream_index == index) {
      if (avcodec_send_packet(context, packet) < 0) {
        frames = -1;
        break;
      }
      const int result = consume();
      if (frames < 0 || (result != AVERROR(EAGAIN) && result != AVERROR_EOF)) {
        frames = -1;
        break;
      }
    }
    av_packet_unref(packet);
  }
  if (demux_result != AVERROR_EOF) frames = -1;
  if (frames >= 0) {
    if (avcodec_send_packet(context, nullptr) < 0 || consume() != AVERROR_EOF)
      frames = -1;
  }
  av_packet_free(&packet);
  av_frame_free(&frame);
  avcodec_free_context(&context);
  avformat_close_input(&format);
  StreamResult result;
  result.frames = frames;
  if (first != AV_NOPTS_VALUE && last != AV_NOPTS_VALUE && last >= first)
    result.seconds = (last - first) * av_q2d(time_base);
  return result;
}

}

LiveDecodeResult decode_live_mpegts(const std::vector<uint8_t> &data) {
  std::string pattern = (std::filesystem::temp_directory_path() / "live-output-XXXXXX").string();
  const char *created = ::mkdtemp(pattern.data());
  if (!created) return {};
  const std::filesystem::path directory(created);
  const auto path = directory / "media.ts";
  std::ofstream(path, std::ios::binary).write(reinterpret_cast<const char *>(data.data()),
                                              static_cast<std::streamsize>(data.size()));
  AudioMetrics metrics;
  const StreamResult video = decode_stream(path, AVMEDIA_TYPE_VIDEO, nullptr);
  const StreamResult audio = decode_stream(path, AVMEDIA_TYPE_AUDIO, &metrics);
  const double rms = metrics.samples == 0 ? 0 : std::sqrt(metrics.energy / metrics.samples);
  const double tone = metrics.samples == 0
                          ? 0
                          : 2 * std::hypot(metrics.real, metrics.imaginary) / metrics.samples;
  LiveDecodeResult result;
  result.video_seconds = video.seconds;
  result.audio_seconds = audio.seconds;
  result.valid = video.frames > 0 && audio.frames > 0 && rms >= 0.001 &&
                 tone >= 0.05 && video.seconds > 0 && audio.seconds > 0 &&
                  std::abs(video.seconds - audio.seconds) < 0.5;
  if (result.valid) std::filesystem::remove_all(directory);
  else std::cerr << "failed strict media retained: " << path << '\n';
  return result;
}
