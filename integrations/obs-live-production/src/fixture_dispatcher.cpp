#include "fixture_dispatcher.hpp"

#include <QCoreApplication>
#include <QMetaObject>
#include <QObject>
#include <QThread>

#include <condition_variable>
#include <mutex>
#include <new>
#include <optional>
#include <utility>

namespace live_production {

struct FixtureTaskDispatcher::Core {
  std::mutex mutex;
  std::condition_variable idle;
  QObject *receiver = nullptr;
  QObject *pending_delete = nullptr;
  Callback callback;
  size_t executing = 0;
  bool closed = true;
  bool paused = false;
  std::optional<uint64_t> held_generation;
#ifdef OBS_LIVE_PRODUCTION_DISPATCHER_TESTING
  std::function<void()> admission_hook;
#endif
};

FixtureTaskDispatcher::FixtureTaskDispatcher() : core_(std::make_shared<Core>()) {}

FixtureTaskDispatcher::~FixtureTaskDispatcher() { close(); }

bool FixtureTaskDispatcher::open(Callback callback) {
  QObject *receiver = create_receiver();
  if (!receiver)
    return false;
  bool accepted = false;
  {
    std::lock_guard<std::mutex> lock(core_->mutex);
    if (core_->closed && !core_->receiver) {
      core_->callback = std::move(callback);
      core_->receiver = receiver;
      core_->closed = false;
      accepted = true;
    }
  }
  if (!accepted)
    delete_receiver(receiver);
  return accepted;
}

bool FixtureTaskDispatcher::enqueue(uint64_t generation) {
  const std::shared_ptr<Core> core = core_;
  std::lock_guard<std::mutex> lock(core->mutex);
  if (core->closed || !core->receiver)
    return false;
#ifdef OBS_LIVE_PRODUCTION_DISPATCHER_TESTING
  if (core->admission_hook)
    core->admission_hook();
#endif
  if (core->paused) {
    if (core->held_generation) return false;
    core->held_generation = generation;
    return true;
  }
  return QMetaObject::invokeMethod(
      core->receiver, [core, generation] { execute(core, generation); },
      Qt::QueuedConnection);
}

bool FixtureTaskDispatcher::pause(bool paused) {
  const std::shared_ptr<Core> core = core_;
  std::lock_guard<std::mutex> lock(core->mutex);
  if (core->closed || !core->receiver) return false;
  core->paused = paused;
  if (paused || !core->held_generation) return true;
  const uint64_t generation = *core->held_generation;
  core->held_generation.reset();
  return QMetaObject::invokeMethod(
      core->receiver, [core, generation] { execute(core, generation); },
      Qt::QueuedConnection);
}

void FixtureTaskDispatcher::close() {
  const std::shared_ptr<Core> core = core_;
  QObject *receiver = nullptr;
  {
    std::unique_lock<std::mutex> lock(core->mutex);
    if (core->closed && !core->receiver)
      return;
    core->closed = true;
    core->paused = false;
    core->held_generation.reset();
    core->callback = {};
    receiver = std::exchange(core->receiver, nullptr);
    const bool receiver_thread =
        receiver && QThread::currentThread() == receiver->thread();
    if (core->executing != 0 && receiver_thread) {
      core->pending_delete = receiver;
      return;
    }
    core->idle.wait(lock, [&core] { return core->executing == 0; });
  }
  delete_receiver(receiver);
}

void FixtureTaskDispatcher::execute(const std::shared_ptr<Core> &core,
                                    uint64_t generation) {
  Callback callback;
  {
    std::lock_guard<std::mutex> lock(core->mutex);
    if (core->closed || !core->receiver)
      return;
    ++core->executing;
    callback = core->callback;
  }
  callback(generation);
  QObject *receiver = nullptr;
  {
    std::lock_guard<std::mutex> lock(core->mutex);
    --core->executing;
    if (core->executing == 0) {
      core->idle.notify_all();
      receiver = std::exchange(core->pending_delete, nullptr);
    }
  }
  if (receiver)
    receiver->deleteLater();
}

QObject *FixtureTaskDispatcher::create_receiver() {
  QCoreApplication *application = QCoreApplication::instance();
  if (!application)
    return nullptr;
  QObject *receiver = nullptr;
  const auto create = [&receiver] { receiver = new (std::nothrow) QObject(); };
  if (QThread::currentThread() == application->thread())
    create();
  else if (!QMetaObject::invokeMethod(application, create,
                                      Qt::BlockingQueuedConnection))
    return nullptr;
  return receiver;
}

void FixtureTaskDispatcher::delete_receiver(QObject *receiver) {
  if (!receiver)
    return;
  if (QThread::currentThread() == receiver->thread()) {
    delete receiver;
    return;
  }
  QCoreApplication *application = QCoreApplication::instance();
  if (!application) {
    receiver->deleteLater();
    return;
  }
  QMetaObject::invokeMethod(application, [receiver] { delete receiver; },
                            Qt::BlockingQueuedConnection);
}

#ifdef OBS_LIVE_PRODUCTION_DISPATCHER_TESTING
void FixtureTaskDispatcher::set_admission_hook(std::function<void()> hook) {
  std::lock_guard<std::mutex> lock(core_->mutex);
  core_->admission_hook = std::move(hook);
}
#endif

}
