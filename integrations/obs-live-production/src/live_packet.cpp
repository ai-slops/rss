#include "live_packet.hpp"

namespace live_production {

LivePacket::LivePacket(encoder_packet *packet) {
  if (packet) {
    packet_ = *packet;
    payload_.assign(packet->data, packet->data + packet->size);
    packet_.data = payload_.data();
  }
}

}
