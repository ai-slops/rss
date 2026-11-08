#include "recovery_fault.hpp"

#include <iostream>
#include <stdexcept>

namespace {

void require(bool value, const char *message) {
  if (!value) throw std::runtime_error(message);
}

void monotonic_allowlisted_commands_only() {
  const std::vector<std::string> allowlist = {"dropPublisher", "manualStopAfterQueuedRestart"};
  require(live_production::accept_recovery_fault(3, 4, "dropPublisher", allowlist),
          "next allowlisted command was rejected");
  require(!live_production::accept_recovery_fault(4, 4, "dropPublisher", allowlist),
          "duplicate command was accepted");
  require(!live_production::accept_recovery_fault(4, 3, "dropPublisher", allowlist),
          "older command was accepted");
  require(!live_production::accept_recovery_fault(4, 5, "freezePublisher", allowlist),
          "non-fixture operation was accepted");
  require(!live_production::accept_recovery_fault(4, 5, "genericCommand", allowlist),
          "generic command was accepted");
}

}

int main() {
  try {
    monotonic_allowlisted_commands_only();
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
