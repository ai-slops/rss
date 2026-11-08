#pragma once

#include <cstdint>
#include <vector>

struct LiveDecodeResult {
  bool valid = false;
  double video_seconds = 0;
  double audio_seconds = 0;
};

LiveDecodeResult decode_live_mpegts(const std::vector<uint8_t> &data);
