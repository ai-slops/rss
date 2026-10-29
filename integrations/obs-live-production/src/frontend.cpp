#include "frontend.hpp"

#include "config.hpp"
#include "publisher.hpp"

#include <obs-frontend-api.h>
#include <obs-module.h>

#include <QAction>
#include <QCheckBox>
#include <QDialog>
#include <QFileDialog>
#include <QFormLayout>
#include <QGroupBox>
#include <QHBoxLayout>
#include <QLabel>
#include <QLineEdit>
#include <QMetaObject>
#include <QObject>
#include <QPointer>
#include <QPushButton>
#include <QString>
#include <QVBoxLayout>
#include <QWidget>

#include <cstdint>
#include <optional>
#include <string>
#include <utility>

namespace live_production {
namespace {

QString text(const char *key) {
  return QString::fromUtf8(obs_module_text(key));
}

QString display_state(const std::string &state) {
  if (state == "ready") return text("State.Ready");
  if (state == "starting") return text("State.Starting");
  if (state == "sending") return text("State.Sending");
  if (state == "reconnecting") return text("State.Reconnecting");
  if (state == "stopping") return text("State.Stopping");
  if (state == "stopped") return text("State.Stopped");
  if (state == "failed") return text("State.Failed");
  return text("State.Unconfigured");
}

bool resolve_source(const PublisherConfig &config, QString &name,
                    std::string &error) {
  obs_source_t *source = config.obs.source_uuid.empty()
                             ? obs_get_source_by_name(config.obs.source_name.c_str())
                             : obs_get_source_by_uuid(config.obs.source_uuid.c_str());
  if (!source) {
    error = "configured OBS source was not found";
    return false;
  }
  const char *actual_name = obs_source_get_name(source);
  name = QString::fromUtf8(actual_name ? actual_name : "");
  obs_source_release(source);
  if (!name.isEmpty())
    return true;
  error = "configured OBS source has no display name";
  return false;
}

bool validate_audio(std::string &error) {
  obs_audio_info info {};
  if (!obs_get_audio_info(&info)) {
    error = "OBS audio is not initialized";
    return false;
  }
  if (info.samples_per_sec != 48000 || info.speakers != SPEAKERS_STEREO) {
    error = "OBS audio must be configured for 48 kHz stereo";
    return false;
  }
  return true;
}

}

class PublisherDialog final : public QDialog {
public:
  explicit PublisherDialog(QWidget *parent) : QDialog(parent) {
    setWindowTitle(text("Dialog.Title"));
    setModal(false);
    resize(620, 420);

    auto *layout = new QVBoxLayout(this);
    auto *intro = new QLabel(text("Dialog.Intro"), this);
    intro->setWordWrap(true);
    layout->addWidget(intro);

    auto *bundle_row = new QHBoxLayout();
    bundle_path_ = new QLineEdit(this);
    bundle_path_->setReadOnly(true);
    bundle_path_->setPlaceholderText(text("Bundle.None"));
    select_button_ = new QPushButton(text("Bundle.Select"), this);
    bundle_row->addWidget(bundle_path_, 1);
    bundle_row->addWidget(select_button_);
    layout->addLayout(bundle_row);

    auto *selection = new QGroupBox(text("Selection.Title"), this);
    auto *selection_layout = new QFormLayout(selection);
    source_value_ = new QLabel(text("Value.Unknown"), selection);
    source_value_->setWordWrap(true);
    mix_value_ = new QLabel(text("Value.Unknown"), selection);
    encoding_value_ = new QLabel(text("Value.Unknown"), selection);
    selection_layout->addRow(text("Selection.Source"), source_value_);
    selection_layout->addRow(text("Selection.Mix"), mix_value_);
    selection_layout->addRow(text("Selection.Encoding"), encoding_value_);
    layout->addWidget(selection);

    confirm_ = new QCheckBox(text("Selection.Confirm"), this);
    layout->addWidget(confirm_);

    auto *status_group = new QGroupBox(text("Status.Title"), this);
    auto *status_layout = new QFormLayout(status_group);
    state_value_ = new QLabel(text("State.Unconfigured"), status_group);
    error_value_ = new QLabel(status_group);
    error_value_->setWordWrap(true);
    error_value_->setTextInteractionFlags(Qt::TextSelectableByMouse);
    status_layout->addRow(text("Status.State"), state_value_);
    status_layout->addRow(text("Status.Error"), error_value_);
    layout->addWidget(status_group);

    auto *note = new QLabel(text("Dialog.CloseNote"), this);
    note->setWordWrap(true);
    layout->addWidget(note);

    auto *actions = new QHBoxLayout();
    actions->addStretch();
    start_button_ = new QPushButton(text("Action.Start"), this);
    stop_button_ = new QPushButton(text("Action.Stop"), this);
    close_button_ = new QPushButton(text("Action.Close"), this);
    actions->addWidget(start_button_);
    actions->addWidget(stop_button_);
    actions->addWidget(close_button_);
    layout->addLayout(actions);

    connect(select_button_, &QPushButton::clicked, this,
            [this] { select_bundle(); });
    connect(confirm_, &QCheckBox::toggled, this,
            [this] { refresh(); });
    connect(start_button_, &QPushButton::clicked, this,
            [this] { start_publisher(); });
    connect(stop_button_, &QPushButton::clicked, this,
            [this] { stop_publisher(); });
    connect(close_button_, &QPushButton::clicked, this, &QDialog::close);
    refresh();
  }

  ~PublisherDialog() override { publisher_.reset(); }

  void refresh() {
    const std::string state = publisher_ ? publisher_->state()
                                         : config_ ? "ready" : "created";
    state_value_->setText(display_state(state));
    const std::string error = publisher_ ? publisher_->last_error() : load_error_;
    error_value_->setText(QString::fromStdString(error));
    const bool active = state == "starting" || state == "sending" ||
                        state == "reconnecting" || state == "stopping";
    select_button_->setEnabled(!active);
    confirm_->setEnabled(config_.has_value() && !active);
    start_button_->setEnabled(config_.has_value() && confirm_->isChecked() &&
                              !active && state != "stopping");
    stop_button_->setEnabled(state == "starting" || state == "sending" ||
                            state == "reconnecting");
  }

private:
  void select_bundle() {
    const QString path = QFileDialog::getOpenFileName(
        this, text("Bundle.DialogTitle"), {}, text("Bundle.Filter"));
    if (path.isEmpty())
      return;
    PublisherConfig candidate;
    std::string error;
    QString actual_source;
    if (!load_publisher_config(path.toStdString(), candidate, error) ||
        !resolve_source(candidate, actual_source, error) ||
        !validate_audio(error)) {
      publisher_.reset();
      config_.reset();
      load_error_ = std::move(error);
      bundle_path_->setText(path);
      source_value_->setText(text("Value.Unknown"));
      mix_value_->setText(text("Value.Unknown"));
      encoding_value_->setText(text("Value.Unknown"));
      confirm_->setChecked(false);
      refresh();
      return;
    }
    publisher_.reset();
    config_ = std::move(candidate);
    load_error_.clear();
    bundle_path_->setText(path);
    source_value_->setText(actual_source);
    mix_value_->setText(
        text("Selection.MixPrefix") +
        QString::number(config_->obs.audio_mix_index + 1) +
        text("Selection.MixSuffix"));
    encoding_value_->setText(
        QString::number(config_->encoding.width) + " × " +
        QString::number(config_->encoding.height) + " · " +
        QString::number(config_->encoding.fps_numerator) + "/" +
        QString::number(config_->encoding.fps_denominator) + " fps · 48 kHz");
    confirm_->setChecked(false);
    refresh();
  }

  void start_publisher() {
    if (!config_ || !confirm_->isChecked())
      return;
    std::string error;
    if (!validate_audio(error)) {
      publisher_.reset();
      load_error_ = std::move(error);
      refresh();
      return;
    }
    if (!publisher_ || publisher_->state() == "failed") {
      const std::string instance = "manual-" + std::to_string(++instance_id_);
      QPointer<PublisherDialog> guard(this);
      publisher_ = std::make_unique<Publisher>(
          *config_, instance, [guard] {
            if (!guard)
              return;
            QMetaObject::invokeMethod(
                guard,
                [guard] {
                  if (guard)
                    guard->refresh();
                },
                Qt::QueuedConnection);
          });
      if (!publisher_->initialize(error)) {
        refresh();
        return;
      }
    }
    publisher_->start(error);
    refresh();
  }

  void stop_publisher() {
    if (publisher_)
      publisher_->stop();
    refresh();
  }

  QLineEdit *bundle_path_ = nullptr;
  QPushButton *select_button_ = nullptr;
  QLabel *source_value_ = nullptr;
  QLabel *mix_value_ = nullptr;
  QLabel *encoding_value_ = nullptr;
  QCheckBox *confirm_ = nullptr;
  QLabel *state_value_ = nullptr;
  QLabel *error_value_ = nullptr;
  QPushButton *start_button_ = nullptr;
  QPushButton *stop_button_ = nullptr;
  QPushButton *close_button_ = nullptr;
  std::optional<PublisherConfig> config_;
  std::unique_ptr<Publisher> publisher_;
  std::string load_error_;
  uint64_t instance_id_ = 0;
};

FrontendController::FrontendController() = default;

FrontendController::~FrontendController() { shutdown(); }

bool FrontendController::install() {
  if (action_)
    return true;
  action_ = static_cast<QAction *>(
      obs_frontend_add_tools_menu_qaction(obs_module_text("Tools.Menu")));
  if (!action_)
    return false;
  QObject::connect(action_, &QObject::destroyed,
                   [this] { action_ = nullptr; });
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
    dialog_ = std::make_unique<PublisherDialog>(parent);
  }
  dialog_->show();
  dialog_->raise();
  dialog_->activateWindow();
}

}
