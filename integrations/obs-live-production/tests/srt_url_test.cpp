#include "srt_url.hpp"

int main() {
  live_production::TargetConfig ipv4;
  ipv4.host = "127.0.0.1";
  ipv4.port = 9000;
  ipv4.buffer_ms = 120;
  if (live_production::build_srt_url(ipv4) !=
      "srt://127.0.0.1:9000?mode=caller&latency=120000")
    return 1;

  live_production::TargetConfig ipv6;
  ipv6.host = "::1";
  ipv6.port = 9001;
  ipv6.buffer_ms = 250;
  if (live_production::build_srt_url(ipv6) !=
      "srt://[::1]:9001?mode=caller&latency=250000")
    return 1;
  return 0;
}
