#pragma once

#include "live_settings.hpp"

namespace live_production {

void register_live_srt_output();
inline bool live_srt_settings_valid(const LiveSrtSettings &srt, const LiveMuxSettings &mux) {
  return !srt.host.empty() && srt.port >= 1 && srt.port <= 65535 &&
         srt.passphrase.size() >= 10 && srt.passphrase.size() <= 79 && srt.latency_ms >= 1 &&
         srt.latency_ms <= 60000 && mux.width != 0 && mux.height != 0 &&
         mux.fps_num != 0 && mux.fps_den != 0;
}

}
