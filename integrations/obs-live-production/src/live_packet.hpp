#pragma once

#include <obs.h>
#include <vector>

namespace live_production {

class LivePacket {
 public:
  explicit LivePacket(encoder_packet *packet);
  LivePacket(const LivePacket &) = delete;
  LivePacket &operator=(const LivePacket &) = delete;
  LivePacket(LivePacket &&) noexcept = default;
  LivePacket &operator=(LivePacket &&) noexcept = default;

  const encoder_packet &get() const { return packet_; }
  size_t size() const { return packet_.size; }

 private:
  encoder_packet packet_{};
  std::vector<uint8_t> payload_;
};

}
