#include "fixture.hpp"
#include "fixture_source.hpp"
#include "frontend.hpp"
#include "module_runtime.hpp"
#include "recovery_fixture.hpp"

#include <obs-frontend-api.h>
#include <obs-module.h>

#include <cstdlib>

OBS_DECLARE_MODULE()
OBS_MODULE_USE_DEFAULT_LOCALE("obs-live-production", "en-US")

namespace {

live_production::ModuleRuntime runtime;
live_production::FixtureController controller;
live_production::RecoveryFixture recovery_fixture(runtime);
live_production::FrontendController frontend(runtime);

void frontend_event(obs_frontend_event event, void *) {
  if (event == OBS_FRONTEND_EVENT_FINISHED_LOADING) {
    std::string error;
    if (!controller.start_from_environment(error) ||
        !recovery_fixture.start_from_environment(error))
      blog(LOG_ERROR, "[obs-live-production] fixture start failed: %s",
           error.c_str());
  } else if (event == OBS_FRONTEND_EVENT_EXIT) {
    frontend.shutdown();
    recovery_fixture.stop();
    runtime.shutdown();
    controller.stop();
  }
}

}

MODULE_EXPORT const char *obs_module_description(void) {
  return "Independent authenticated SRT publishing for selected OBS sources";
}

bool obs_module_load(void) {
  live_production::register_fixture_source();
  const char *fixture_path = std::getenv("LIVE_PRODUCTION_OBS_FIXTURE_FILE");
  const char *recovery_path =
      std::getenv("LIVE_PRODUCTION_OBS_RECOVERY_FIXTURE_FILE");
  if (fixture_path && *fixture_path && recovery_path && *recovery_path) {
    blog(LOG_ERROR,
         "[obs-live-production] standard and recovery fixtures are mutually exclusive");
    return false;
  }
  if ((!fixture_path || !*fixture_path) && (!recovery_path || !*recovery_path) &&
      !frontend.install()) {
    blog(LOG_ERROR, "[obs-live-production] failed to install OBS Tools action");
    return false;
  }
  obs_frontend_add_event_callback(frontend_event, nullptr);
  blog(LOG_INFO, "[obs-live-production] module loaded");
  return true;
}

void obs_module_unload(void) {
  obs_frontend_remove_event_callback(frontend_event, nullptr);
  frontend.shutdown();
  recovery_fixture.stop();
  runtime.shutdown();
  controller.stop();
}
