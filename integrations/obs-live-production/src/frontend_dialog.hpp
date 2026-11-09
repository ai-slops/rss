#pragma once

#include <QDialog>
#include <memory>

class QCheckBox;
class QLabel;
class QLineEdit;
class QPushButton;
class QTimer;
class QWidget;

namespace live_production {

class ModuleRuntime;
class AllSourcesPublisher;

class PublisherDialog final : public QDialog {
public:
  PublisherDialog(QWidget *parent, ModuleRuntime &runtime);
  ~PublisherDialog() override;

private:
  void select_bundle();
  void start_publisher();
  void stop_publisher();
  void refresh();

  ModuleRuntime &runtime_;
  std::unique_ptr<AllSourcesPublisher> all_sources_;
  QLabel *all_status_ = nullptr;
  QPushButton *all_start_ = nullptr;
  QPushButton *all_stop_ = nullptr;
  QLineEdit *bundle_path_ = nullptr;
  QPushButton *select_button_ = nullptr;
  QLabel *source_value_ = nullptr;
  QLabel *mix_value_ = nullptr;
  QLabel *encoding_value_ = nullptr;
  QCheckBox *confirm_ = nullptr;
  QLabel *state_value_ = nullptr;
  QLabel *recovery_value_ = nullptr;
  QLabel *error_value_ = nullptr;
  QPushButton *start_button_ = nullptr;
  QPushButton *stop_button_ = nullptr;
  QTimer *refresh_timer_ = nullptr;
};

}
