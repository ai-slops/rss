#include "live_srt_transport.hpp"
#include "live_address_plan.hpp"

#include <QCoreApplication>

#include <chrono>
#include <condition_variable>
#include <iostream>
#include <mutex>
#include <stdexcept>
#include <thread>

namespace {

void require(bool value, const char *message) {
  if (!value) throw std::runtime_error(message);
}

live_production::LiveSrtSettings settings(const char *host) {
  live_production::LiveSrtSettings value;
  value.host = host;
  value.port = 9;
  value.passphrase = "fixtureCredential42";
  value.stream_id = "transport-test";
  value.latency_ms = 120;
  return value;
}

}

int main(int argc, char **argv) {
  QCoreApplication application(argc, argv);
  try {
    auto now = std::chrono::steady_clock::time_point{};
    const auto deadline = now + std::chrono::seconds(3);
    const std::vector<QHostAddress> candidates{QHostAddress("::1"), QHostAddress("127.0.0.1")};
    int attempts = 0;
    const bool fallback = live_production::connect_live_addresses(
        live_production::LiveAddressPlan{candidates, deadline}, [&] { return now; },
        [&](const QHostAddress &, int budget) {
          now += std::chrono::milliseconds(budget);
          return ++attempts == 2;
        });
    require(fallback && attempts == 2 && now <= deadline,
            "first address timeout starved the valid fallback");
    const auto no_listener_started = std::chrono::steady_clock::now();
    live_production::LiveSrtTransport no_listener(settings("localhost"));
    require(!no_listener.connect(), "unexpected no-listener connection");
    require(std::chrono::steady_clock::now() - no_listener_started <
                std::chrono::seconds(4),
            "localhost attempts exceeded total budget");

    live_production::LiveSrtTransport pending(settings("no-such-live-output.invalid"));
    std::mutex mutex;
    std::condition_variable changed;
    bool entered = false;
    bool connected = true;
    std::thread worker([&] {
      {
        std::lock_guard<std::mutex> lock(mutex);
        entered = true;
        changed.notify_one();
      }
      connected = pending.connect();
    });
    {
      std::unique_lock<std::mutex> lock(mutex);
      changed.wait(lock, [&] { return entered; });
    }
    const auto cancel_started = std::chrono::steady_clock::now();
    pending.interrupt();
    worker.join();
    require(!connected, "cancelled lookup connected");
    require(std::chrono::steady_clock::now() - cancel_started <
                std::chrono::seconds(2),
            "cancelled lookup exceeded Stop deadline");
    std::cout << "live transport tests passed\n";
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
