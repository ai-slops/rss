#include "srt_url.hpp"

namespace live_production {

std::string build_srt_url(const TargetConfig &target) {
  const bool ipv6 = target.host.find(':') != std::string::npos &&
                    target.host.front() != '[';
  const std::string host = ipv6 ? "[" + target.host + "]" : target.host;
  const uint64_t latency_us = static_cast<uint64_t>(target.buffer_ms) * 1000;
  return "srt://" + host + ":" + std::to_string(target.port) +
         "?mode=caller&latency=" + std::to_string(latency_us);
}

}
