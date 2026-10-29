#include "module_runtime.hpp"

#include <obs-module.h>
#include <QCoreApplication>
#include <QFile>
#include <QTemporaryDir>

#include <iostream>
#include <stdexcept>

OBS_DECLARE_MODULE()

namespace {

void require(bool value, const char *message) {
  if (!value) throw std::runtime_error(message);
}

void failed_configuration_cannot_leave_new_target_startable() {
  live_production::ModuleRuntime runtime;
  std::string error;
  require(!runtime.configure("/nonexistent/private-publisher.json", error),
          "invalid configuration was accepted");
  require(!runtime.snapshot().config.has_value(),
          "failed configuration left publisher configured");
  error.clear();
  require(!runtime.manual_start(error),
          "manual Start succeeded after failed configuration");
  require(runtime.snapshot().publisher_state == "created",
          "failed configuration left publisher ready");
}

void late_recovery_setup_failure_clears_publisher_configuration() {
  QTemporaryDir directory;
  require(directory.isValid(), "private temporary directory unavailable");
  const QString bundle_path = directory.filePath("publisher.json");
  QFile bundle(bundle_path);
  require(bundle.open(QIODevice::WriteOnly), "cannot create private bundle");
  require(bundle.setPermissions(QFileDevice::ReadOwner | QFileDevice::WriteOwner),
          "cannot secure private bundle");
  const QByteArray json = R"json({"schemaVersion":1,
    "target":{"sourceId":"source","videoTrackId":"video","audioTrackId":"audio",
      "host":"127.0.0.1","port":9000,"passphrase":"scoped-test-passphrase","streamId":"source-test","bufferMs":120},
    "obs":{"sourceName":"Camera","audioMixIndex":0},
    "encoding":{"width":1280,"height":720,"fpsNumerator":30,"fpsDenominator":1,"audioSampleRate":48000,"videoBitrateKbps":2500},
    "recovery":{"schemaVersion":1,"controlUrl":"https://[invalid/api/v1/source-recovery/source/poll",
      "controllerId":"controller","controllerToken":"0123456789abcdef"}})json";
  require(bundle.write(json) == json.size(), "cannot write private bundle");
  bundle.close();
  live_production::ModuleRuntime runtime(
      directory.filePath("nested/module/recovery").toStdString());
  std::string error;
  require(!runtime.configure(bundle_path.toStdString(), error),
          "malformed recovery URL reached a configured runtime");
  require(error == "recovery control URL must be HTTPS or loopback HTTP",
          "test did not reach late network setup rejection");
  const auto snapshot = runtime.snapshot();
  require(!snapshot.config && snapshot.bundle_path.empty() &&
              snapshot.publisher_state == "created",
          "late setup failure retained a startable publisher target");
  require(!runtime.manual_start(error), "rejected target remained startable");
}

}

int main(int argc, char **argv) {
  QCoreApplication application(argc, argv);
  try {
    failed_configuration_cannot_leave_new_target_startable();
    late_recovery_setup_failure_clears_publisher_configuration();
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
