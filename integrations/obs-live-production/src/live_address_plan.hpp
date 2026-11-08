#pragma once

#include <QHostAddress>

#include <algorithm>
#include <chrono>
#include <vector>

namespace live_production {

struct LiveAddressPlan {
  const std::vector<QHostAddress> &addresses;
  std::chrono::steady_clock::time_point deadline;
};

template <class Now, class Attempt>
bool connect_live_addresses(const LiveAddressPlan &plan, Now now, Attempt attempt) {
  std::vector<QHostAddress> addresses;
  for (const auto &address : plan.addresses) {
    if (address.protocol() != QAbstractSocket::IPv4Protocol &&
        address.protocol() != QAbstractSocket::IPv6Protocol) continue;
    if (std::find(addresses.begin(), addresses.end(), address) == addresses.end())
      addresses.push_back(address);
    if (addresses.size() == 8) break;
  }
  for (size_t index = 0; index < addresses.size(); ++index) {
    const auto remaining = std::chrono::duration_cast<std::chrono::milliseconds>(
        plan.deadline - now());
    if (remaining.count() <= 0) return false;
    const auto left = static_cast<int64_t>(addresses.size() - index);
    const auto budget = std::max<int64_t>(1, remaining.count() / left);
    if (attempt(addresses[index], static_cast<int>(budget))) return true;
  }
  return false;
}

}
