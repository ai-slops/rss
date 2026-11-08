#pragma once

#include <cstdint>
#include <string>
#include <vector>

namespace live_production {

struct SenderEvidence {
  std::string source_id;
  std::string source_name;
  uint32_t audio_mix_index = 0;
  std::string state;
  std::string error;
  std::string color;
  bool frame_counter = false;
  uint32_t tone_hz = 0;
};

struct NegativeEvidence {
  std::string kind;
  std::string result;
};

bool write_report(const std::string &path,
                  const std::string &fixture_state,
                  const std::vector<SenderEvidence> &senders,
                  const std::vector<NegativeEvidence> &negative_cases,
                  std::string &error);

}
