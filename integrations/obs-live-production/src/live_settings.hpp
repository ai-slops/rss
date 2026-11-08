#pragma once

#include <cstdint>
#include <string>

namespace live_production {

struct LiveSrtSettings {
  std::string host;
  int port = 0;
  std::string passphrase;
  std::string stream_id;
  int latency_ms = 120;
};

struct LiveMuxSettings {
  uint32_t width = 1280;
  uint32_t height = 720;
  uint32_t fps_num = 30;
  uint32_t fps_den = 1;
};

}
