#pragma once

#include <cstdint>
#include <string>
#include <vector>

namespace live_production {

const std::vector<std::string> &recovery_fault_kinds();
bool accept_recovery_fault(uint64_t previous_id, uint64_t command_id,
                           const std::string &kind,
                           const std::vector<std::string> &allowlist);

}
