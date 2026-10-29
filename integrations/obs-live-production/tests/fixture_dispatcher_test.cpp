#include "fixture_dispatcher.hpp"

#include <QCoreApplication>
#include <QEventLoop>
#include <QMetaObject>
#include <QObject>

#include <atomic>
#include <condition_variable>
#include <cstdint>
#include <iostream>
#include <mutex>
#include <stdexcept>
#include <thread>

namespace {

void require(bool condition, const char *message) {
  if (!condition)
    throw std::runtime_error(message);
}

void paused_enqueue_cannot_survive_stop() {
  // Given
  live_production::FixtureTaskDispatcher dispatcher;
  std::atomic<size_t> callbacks = 0;
  require(dispatcher.open([&](uint64_t) { callbacks.fetch_add(1); }),
          "dispatcher open failed");
  std::mutex pause_mutex;
  std::condition_variable pause_condition;
  bool admission_entered = false;
  bool release_admission = false;
  dispatcher.set_admission_hook([&] {
    std::unique_lock<std::mutex> lock(pause_mutex);
    admission_entered = true;
    pause_condition.notify_all();
    pause_condition.wait(lock, [&] { return release_admission; });
  });
  bool accepted = false;
  std::thread enqueue([&] { accepted = dispatcher.enqueue(7); });
  {
    std::unique_lock<std::mutex> lock(pause_mutex);
    pause_condition.wait(lock, [&] { return admission_entered; });
  }
  std::atomic<bool> stop_started = false;
  std::atomic<bool> stop_finished = false;
  std::thread stop([&] {
    stop_started.store(true);
    dispatcher.close();
    stop_finished.store(true);
  });
  while (!stop_started.load())
    std::this_thread::yield();

  // When
  require(!stop_finished.load(), "stop passed a paused admission");
  {
    std::lock_guard<std::mutex> lock(pause_mutex);
    release_admission = true;
  }
  pause_condition.notify_all();
  enqueue.join();
  while (!stop_finished.load()) {
    QCoreApplication::processEvents(QEventLoop::AllEvents);
    std::this_thread::yield();
  }
  stop.join();
  QCoreApplication::processEvents(QEventLoop::AllEvents);

  // Then
  require(accepted, "paused enqueue was not admitted before stop");
  require(callbacks.load() == 0, "queued callback survived receiver deletion");
  require(!dispatcher.enqueue(8), "enqueue succeeded after dispatcher close");
}

void close_does_not_drain_unrelated_meta_calls() {
  // Given
  live_production::FixtureTaskDispatcher dispatcher;
  std::atomic<size_t> fixture_callbacks = 0;
  size_t unrelated_callbacks = 0;
  require(dispatcher.open([&](uint64_t) { fixture_callbacks.fetch_add(1); }),
          "dispatcher open failed");
  require(dispatcher.enqueue(1), "fixture enqueue failed");
  QObject unrelated;
  require(QMetaObject::invokeMethod(
              &unrelated, [&] { ++unrelated_callbacks; }, Qt::QueuedConnection),
          "unrelated enqueue failed");

  // When
  dispatcher.close();
  require(unrelated_callbacks == 0,
          "fixture close synchronously drained unrelated Qt callbacks");
  QCoreApplication::processEvents(QEventLoop::AllEvents);

  // Then
  require(fixture_callbacks.load() == 0,
          "fixture callback survived receiver deletion");
  require(unrelated_callbacks == 1, "unrelated Qt callback was drained");
}

}

int main(int argc, char **argv) {
  QCoreApplication application(argc, argv);
  try {
    paused_enqueue_cannot_survive_stop();
    close_does_not_drain_unrelated_meta_calls();
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
