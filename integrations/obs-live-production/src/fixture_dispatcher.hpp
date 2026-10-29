#pragma once

#include <cstdint>
#include <functional>
#include <memory>

class QObject;

namespace live_production {

class FixtureTaskDispatcher {
public:
  using Callback = std::function<void(uint64_t)>;

  FixtureTaskDispatcher();
  ~FixtureTaskDispatcher();
  FixtureTaskDispatcher(const FixtureTaskDispatcher &) = delete;
  FixtureTaskDispatcher &operator=(const FixtureTaskDispatcher &) = delete;

  bool open(Callback callback);
  bool enqueue(uint64_t generation);
  void close();

#ifdef OBS_LIVE_PRODUCTION_DISPATCHER_TESTING
  void set_admission_hook(std::function<void()> hook);
#endif

private:
  struct Core;

  static void execute(const std::shared_ptr<Core> &core, uint64_t generation);
  static QObject *create_receiver();
  static void delete_receiver(QObject *receiver);

  std::shared_ptr<Core> core_;
};

}
