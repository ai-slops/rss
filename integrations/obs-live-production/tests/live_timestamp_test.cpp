#include "live_muxer.hpp"
#include "live_srt_output.hpp"

extern "C" {
#include <libavutil/avutil.h>
#include <libavutil/mathematics.h>
}

#include <iostream>
#include <stdexcept>

namespace {

void require(bool value, const char *message) {
  if (!value) throw std::runtime_error(message);
}

}

int main() {
  try {
    const AVRational ntsc{1001, 30000};
    const AVRational clock90k{1, 90000};
    const int64_t old_direct = av_rescale_q(1001, ntsc, clock90k);
    const int64_t corrected =
        live_production::rescale_obs_timestamp(1001, ntsc, clock90k);
    require(old_direct == 3006003, "old NTSC value changed unexpectedly");
    require(corrected == 3003, "OBS NTSC timestamp was not normalized");
    require(live_production::rescale_obs_timestamp(-1001, ntsc, clock90k) == -3003,
            "negative DTS was not preserved");
    require(live_production::rescale_obs_timestamp(AV_NOPTS_VALUE, ntsc, clock90k) ==
                AV_NOPTS_VALUE,
            "NOPTS sentinel was not preserved");
    require(live_production::rescale_obs_timestamp(1024, AVRational{1, 48000},
                                                   clock90k) == 1920,
            "unit-rate audio timestamp changed");
    live_production::LiveSrtSettings target{"localhost", 1, "fixtureCredential42", "stream", 1};
    const live_production::LiveMuxSettings encoding{64, 64, 30, 1};
    require(live_production::live_srt_settings_valid(target, encoding),
            "minimum latency was rejected");
    target.port = 65535;
    target.latency_ms = 60000;
    require(live_production::live_srt_settings_valid(target, encoding),
            "maximum latency was rejected");
    target.latency_ms = 0;
    require(!live_production::live_srt_settings_valid(target, encoding), "zero latency was accepted");
    target.latency_ms = 60001;
    require(!live_production::live_srt_settings_valid(target, encoding), "excess latency was accepted");
    std::cout << "timestamp old=" << old_direct << " corrected=" << corrected
              << " audio=1920\n";
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
