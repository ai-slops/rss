#pragma once

#ifdef _WIN32
#include <winsock2.h>
#include <ws2tcpip.h>
#else
#include <sys/socket.h>
#endif

namespace live_production {

struct LiveSrtRequest {
  sockaddr_storage address{};
  int address_length = 0;
  int timeout_ms = 0;
  int latency_ms = 0;
  char passphrase[80]{};
  char stream_id[513]{};
};

}
