#pragma once

#include <memory>

class QAction;

namespace live_production {

class PublisherDialog;

class FrontendController {
public:
  FrontendController();
  ~FrontendController();

  FrontendController(const FrontendController &) = delete;
  FrontendController &operator=(const FrontendController &) = delete;

  bool install();
  void shutdown();

private:
  void show_dialog();

  QAction *action_ = nullptr;
  std::unique_ptr<PublisherDialog> dialog_;
};

}
