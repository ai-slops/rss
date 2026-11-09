#include "live_srt_wire.hpp"

#include <srt/srt.h>
#include <array>
#include <algorithm>
#include <chrono>
#include <thread>
#include <cerrno>
#include <csignal>
#include <cstring>
#include <fcntl.h>
#include <sys/prctl.h>
#include <unistd.h>

namespace {

bool reply(bool success) {
  const char value = success ? 1 : 0;
  return ::send(STDIN_FILENO, &value, 1, MSG_NOSIGNAL) == 1;
}

bool configure(SRTSOCKET socket, const live_production::LiveSrtRequest &request) {
  const int yes = 1, timeout = 500, payload = 1316, live = SRTT_LIVE, key_length = 16;
  const auto option = [&](SRT_SOCKOPT name, const void *value, int size) {
    return srt_setsockopt(socket, 0, name, value, size) != SRT_ERROR;
  };
  return option(SRTO_REUSEADDR, &yes, sizeof(yes)) &&
         option(SRTO_TRANSTYPE, &live, sizeof(live)) &&
         option(SRTO_SENDER, &yes, sizeof(yes)) &&
         option(SRTO_SNDTIMEO, &timeout, sizeof(timeout)) &&
         option(SRTO_CONNTIMEO, &request.timeout_ms, sizeof(request.timeout_ms)) &&
         option(SRTO_PAYLOADSIZE, &payload, sizeof(payload)) &&
         option(SRTO_TSBPDMODE, &yes, sizeof(yes)) &&
         option(SRTO_LATENCY, &request.latency_ms, sizeof(request.latency_ms)) &&
         option(SRTO_PBKEYLEN, &key_length, sizeof(key_length)) &&
         option(SRTO_PASSPHRASE, request.passphrase,
                static_cast<int>(std::strlen(request.passphrase))) &&
         option(SRTO_STREAMID, request.stream_id,
                static_cast<int>(std::strlen(request.stream_id)));
}

SRTSOCKET connect(live_production::LiveSrtRequest request) {
  using namespace std::chrono;
  const auto deadline = steady_clock::now() + milliseconds(std::max(1, request.timeout_ms - 20));
  sockaddr_storage local{};
  local.ss_family = request.address.ss_family;
  while (steady_clock::now() < deadline) {
    request.timeout_ms = std::max(1, static_cast<int>(duration_cast<milliseconds>(deadline - steady_clock::now()).count()));
    const SRTSOCKET socket = srt_create_socket();
    int length = request.address_length;
    if (socket == SRT_INVALID_SOCK) break;
    if (!configure(socket, request) ||
        srt_bind(socket, reinterpret_cast<sockaddr *>(&local), length) == SRT_ERROR ||
        srt_getsockname(socket, reinterpret_cast<sockaddr *>(&local), &length) == SRT_ERROR) {
      srt_close(socket);
      break;
    }
    if (srt_connect(socket, reinterpret_cast<const sockaddr *>(&request.address),
                    request.address_length) != SRT_ERROR) return socket;
    int reason = srt_getrejectreason(socket);
    srt_close(socket);
    if (reason >= 1000 && reason < 2000) reason -= 1000;
    if (reason != SRT_REJ_UNKNOWN && reason != SRT_REJ_PEER &&
        reason != SRT_REJ_BACKLOG && reason != SRT_REJ_CLOSE) break;
    // A previous peer can still be closing. Keep this caller's UDP identity
    // and the original shared deadline; bad credentials are never retried.
    std::this_thread::sleep_for(milliseconds(100));
  }
  return SRT_INVALID_SOCK;
}

}

int main() {
  // The private socket is the sole input; no credentials occur in argv or URLs.
  const pid_t parent = getppid();
  if (parent == 1 || prctl(PR_SET_PDEATHSIG, SIGTERM) != 0 || getppid() != parent)
    return 1;
  const int flags = fcntl(STDIN_FILENO, F_GETFL);
  if (flags < 0 || fcntl(STDIN_FILENO, F_SETFL, flags & ~O_NONBLOCK) != 0) return 1;
  live_production::LiveSrtRequest request;
  if (recv(STDIN_FILENO, &request, sizeof(request), MSG_TRUNC) != sizeof(request) ||
      request.address_length <= 0 ||
      request.address_length > static_cast<int>(sizeof(request.address)) ||
      request.passphrase[sizeof(request.passphrase) - 1] != '\0' ||
      request.stream_id[sizeof(request.stream_id) - 1] != '\0' ||
      srt_startup() < 0)
    return 1;
  const SRTSOCKET socket = connect(request);
  bool ready = socket != SRT_INVALID_SOCK;
  if (!reply(ready)) ready = false;
  std::array<char, 1316> bytes{};
  while (ready) {
    const ssize_t received = recv(STDIN_FILENO, bytes.data(), bytes.size(), MSG_TRUNC);
    if (received < 0 && errno == EINTR) continue;
    if (received <= 0 || received > static_cast<ssize_t>(bytes.size())) break;
    SRT_MSGCTRL control = srt_msgctrl_default;
    ready = srt_sendmsg2(socket, bytes.data(), static_cast<int>(received), &control) == received;
    if (!reply(ready)) break;
  }
  if (socket != SRT_INVALID_SOCK) {
    const auto deadline = std::chrono::steady_clock::now() + std::chrono::milliseconds(100);
    while (std::chrono::steady_clock::now() < deadline) {
      int pending = 0, length = sizeof(pending);
      if (srt_getsockopt(socket, 0, SRTO_SNDDATA, &pending, &length) == SRT_ERROR || pending == 0) break;
      std::this_thread::sleep_for(std::chrono::milliseconds(5));
    }
    srt_close(socket);
    // Keep the final shutdown-control exchange alive before last-reference cleanup.
    std::this_thread::sleep_for(std::chrono::milliseconds(100));
  }
  srt_cleanup();
  return ready ? 0 : 1;
}
