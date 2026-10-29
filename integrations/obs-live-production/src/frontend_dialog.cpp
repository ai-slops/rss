#include "frontend_dialog.hpp"

#include "config.hpp"
#include "module_runtime.hpp"

#include <obs-module.h>

#include <QCheckBox>
#include <QFileDialog>
#include <QFormLayout>
#include <QGroupBox>
#include <QHBoxLayout>
#include <QLabel>
#include <QLineEdit>
#include <QPushButton>
#include <QTimer>
#include <QVBoxLayout>

namespace live_production {
namespace {

QString text(const char *key) { return QString::fromUtf8(obs_module_text(key)); }

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

}

PublisherDialog::PublisherDialog(QWidget *parent, ModuleRuntime &runtime)
    : QDialog(parent), runtime_(runtime) {
  setWindowTitle(text("Dialog.Title"));
  setModal(false);
  resize(620, 460);
  auto *layout = new QVBoxLayout(this);
  auto *intro = new QLabel(text("Dialog.Intro"), this);
  intro->setWordWrap(true);
  layout->addWidget(intro);
  auto *bundle_row = new QHBoxLayout();
  bundle_path_ = new QLineEdit(this);
  bundle_path_->setReadOnly(true);
  select_button_ = new QPushButton(text("Bundle.Select"), this);
  bundle_row->addWidget(bundle_path_, 1);
  bundle_row->addWidget(select_button_);
  layout->addLayout(bundle_row);
  auto *selection = new QGroupBox(text("Selection.Title"), this);
  auto *selection_layout = new QFormLayout(selection);
  source_value_ = new QLabel(text("Value.Unknown"), selection);
  mix_value_ = new QLabel(text("Value.Unknown"), selection);
  encoding_value_ = new QLabel(text("Value.Unknown"), selection);
  selection_layout->addRow(text("Selection.Source"), source_value_);
  selection_layout->addRow(text("Selection.Mix"), mix_value_);
  selection_layout->addRow(text("Selection.Encoding"), encoding_value_);
  layout->addWidget(selection);
  confirm_ = new QCheckBox(text("Selection.Confirm"), this);
  layout->addWidget(confirm_);
  auto *status = new QGroupBox(text("Status.Title"), this);
  auto *status_layout = new QFormLayout(status);
  state_value_ = new QLabel(status);
  recovery_value_ = new QLabel(status);
  recovery_value_->setWordWrap(true);
  error_value_ = new QLabel(status);
  error_value_->setWordWrap(true);
  status_layout->addRow(text("Status.State"), state_value_);
  status_layout->addRow(text("Status.Recovery"), recovery_value_);
  status_layout->addRow(text("Status.Error"), error_value_);
  layout->addWidget(status);
  auto *actions = new QHBoxLayout();
  actions->addStretch();
  start_button_ = new QPushButton(text("Action.Start"), this);
  stop_button_ = new QPushButton(text("Action.Stop"), this);
  auto *close = new QPushButton(text("Action.Close"), this);
  actions->addWidget(start_button_);
  actions->addWidget(stop_button_);
  actions->addWidget(close);
  layout->addLayout(actions);
  connect(select_button_, &QPushButton::clicked, this,
          [this] { select_bundle(); });
  connect(confirm_, &QCheckBox::toggled, this, [this] { refresh(); });
  connect(start_button_, &QPushButton::clicked, this,
          [this] { start_publisher(); });
  connect(stop_button_, &QPushButton::clicked, this,
          [this] { stop_publisher(); });
  connect(close, &QPushButton::clicked, this, &QDialog::close);
  refresh_timer_ = new QTimer(this);
  connect(refresh_timer_, &QTimer::timeout, this, [this] { refresh(); });
  refresh_timer_->start(250);
  refresh();
}

void PublisherDialog::select_bundle() {
  const QString path = QFileDialog::getOpenFileName(
      this, text("Bundle.DialogTitle"), {}, text("Bundle.Filter"));
  if (path.isEmpty()) return;
  confirm_->setChecked(false);
  std::string error;
  if (!runtime_.configure(path.toStdString(), error)) {
    error_value_->setText(QString::fromStdString(error));
    refresh();
    return;
  }
  refresh();
}

void PublisherDialog::start_publisher() {
  if (!confirm_->isChecked()) return;
  std::string error;
  runtime_.manual_start(error);
  refresh();
}

void PublisherDialog::stop_publisher() {
  std::string error;
  runtime_.manual_stop(error);
  refresh();
}

void PublisherDialog::refresh() {
  const ModuleRuntimeSnapshot snapshot = runtime_.snapshot();
  bundle_path_->setText(QString::fromStdString(snapshot.bundle_path));
  state_value_->setText(display_state(snapshot.publisher_state));
  error_value_->setText(QString::fromStdString(snapshot.error));
  const bool configured = snapshot.config.has_value();
  bool selection_valid = false;
  if (configured) {
    obs_source_t *source = snapshot.config->obs.source_uuid.empty()
        ? obs_get_source_by_name(snapshot.config->obs.source_name.c_str())
        : obs_get_source_by_uuid(snapshot.config->obs.source_uuid.c_str());
    source_value_->setText(source ? QString::fromUtf8(obs_source_get_name(source))
                                  : text("Value.Unknown"));
    obs_audio_info audio {};
    selection_valid = source && obs_get_audio_info(&audio) &&
                      audio.samples_per_sec == 48000 && audio.speakers == SPEAKERS_STEREO;
    if (source) obs_source_release(source);
    if (!selection_valid && snapshot.error.empty())
      error_value_->setText("Select an available OBS source and configure 48 kHz stereo.");
    mix_value_->setText(QString::number(snapshot.config->obs.audio_mix_index + 1));
    encoding_value_->setText(QString::number(snapshot.config->encoding.width) + " × " +
                             QString::number(snapshot.config->encoding.height));
  } else {
    source_value_->setText(text("Value.Unknown"));
    mix_value_->setText(text("Value.Unknown"));
    encoding_value_->setText(text("Value.Unknown"));
  }
  const QString recovery = snapshot.recovery_enrolled
                               ? QString("%1 · %2 · %3 · %4 · attempts %5 · intent %6")
                                     .arg(QString::fromStdString(snapshot.recovery_desired))
                                     .arg(snapshot.recovery_online ? "controller online"
                                                                   : "controller offline")
                                     .arg(QString::fromStdString(snapshot.receiver_state))
                                     .arg(QString::fromStdString(snapshot.retry_result))
                                     .arg(snapshot.retry_attempts)
                                      .arg(static_cast<qulonglong>(snapshot.intent_epoch))
                               : text("Recovery.Disabled");
  recovery_value_->setText(
      snapshot.start_pending ? recovery + " · Start queued for registration"
                             : snapshot.recovery_enrolled && !snapshot.recovery_ready
                                   ? recovery + " · registering stopped controller"
                                   : recovery);
  const bool active = snapshot.publisher_state == "starting" ||
                      snapshot.publisher_state == "sending" ||
                      snapshot.publisher_state == "reconnecting" ||
                      snapshot.publisher_state == "stopping";
  select_button_->setEnabled(!active);
  confirm_->setEnabled(configured && selection_valid && !active);
  start_button_->setEnabled(configured && selection_valid && confirm_->isChecked() && !active &&
                            !snapshot.start_pending);
  stop_button_->setEnabled(active || snapshot.recovery_desired == "running");
}

}
