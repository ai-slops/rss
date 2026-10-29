#pragma once

#include <memory>

class QAction;

namespace live_production {

class PublisherDialog;
class ModuleRuntime;

class FrontendController {
public:
  explicit FrontendController(ModuleRuntime &runtime);
  ~FrontendController();

  FrontendController(const FrontendController &) = delete;
  FrontendController &operator=(const FrontendController &) = delete;

  bool install();
  void shutdown();

private:
  void show_dialog();

  QAction *action_ = nullptr;
  std::unique_ptr<PublisherDialog> dialog_;
  ModuleRuntime &runtime_;
};

}
