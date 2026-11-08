#include "frontend.hpp"

#include "frontend_dialog.hpp"

#include <obs-frontend-api.h>
#include <obs-module.h>

#include <QAction>
#include <QObject>
#include <QWidget>

namespace live_production {

FrontendController::FrontendController(ModuleRuntime &runtime) : runtime_(runtime) {}

FrontendController::~FrontendController() { shutdown(); }

bool FrontendController::install() {
  if (action_) return true;
  action_ = static_cast<QAction *>(
      obs_frontend_add_tools_menu_qaction(obs_module_text("Tools.Menu")));
  if (!action_) return false;
  QObject::connect(action_, &QObject::destroyed, [this] { action_ = nullptr; });
  QObject::connect(action_, &QAction::triggered, [this] { show_dialog(); });
  return true;
}

void FrontendController::shutdown() {
  dialog_.reset();
  if (action_) {
    QAction *action = action_;
    action_ = nullptr;
    delete action;
  }
}

void FrontendController::show_dialog() {
  if (!dialog_) {
    auto *parent = static_cast<QWidget *>(obs_frontend_get_main_window());
    dialog_ = std::make_unique<PublisherDialog>(parent, runtime_);
  }
  dialog_->show();
  dialog_->raise();
  dialog_->activateWindow();
}

}
