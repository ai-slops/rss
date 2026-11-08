#pragma once

#include "recovery_model.hpp"

#include <cstdint>
#include <functional>
#include <optional>
#include <string>

namespace live_production {

class RecoveryActionHandler {
public:
  using Persist = std::function<bool(std::string &)>;
  using Enqueue = std::function<bool(uint64_t)>;
  using Restart = std::function<bool(std::string &)>;

  bool accept(RecoveryModel &model, const RecoveryAction &action,
              const Persist &persist, const Enqueue &enqueue,
              std::string &error);
  bool handle(ActionDecision decision, RecoveryModel &model,
              const RecoveryAction &action, const Persist &persist,
              const Enqueue &enqueue, std::string &error);
  void execute(RecoveryModel &model, uint64_t intent_epoch,
               const Persist &persist, const Restart &restart,
               std::string &error);
  void cancel();
  bool cancel_pending(RecoveryModel &model, const Persist &persist,
                      std::string &error);

  uint64_t execution_count() const { return execution_count_; }
  bool pending() const { return pending_action_.has_value(); }
  std::string pending_server_run_id() const;

private:
  void fail_closed(RecoveryModel &model, const Persist &persist,
                   std::string &error);

  std::optional<RecoveryAction> pending_action_;
  uint64_t execution_count_ = 0;
};

}
