#include "recovery_client.hpp"

#include <QJsonDocument>
#include <QNetworkAccessManager>
#include <QNetworkReply>
#include <QNetworkRequest>
#include <QObject>
#include <QTimer>
#include <QUrl>
#include <QVariant>

#include <utility>

namespace live_production {

RecoveryClient::RecoveryClient() {
  network_ = new QNetworkAccessManager();
  timer_ = new QTimer();
  timer_->setSingleShot(true);
  QObject::connect(timer_, &QTimer::timeout, [this] { poll(); });
}

RecoveryClient::~RecoveryClient() {
  stop();
  delete timer_;
  delete network_;
}

bool RecoveryClient::configure(const RecoveryConfig &config,
                               RequestBuilder request_builder,
                               ResponseHandler response_handler,
                               ErrorHandler error_handler,
                               std::string &error) {
  const QUrl url(QString::fromStdString(config.control_url));
  const bool loopback = url.host() == "localhost" || url.host() == "127.0.0.1" ||
                        url.host() == "::1";
  if (!url.isValid() || url.hasQuery() || url.hasFragment() || !url.userInfo().isEmpty() ||
      (url.scheme() != "https" && !(url.scheme() == "http" && loopback))) {
    error = "recovery control URL must be HTTPS or loopback HTTP";
    return false;
  }
  config_ = config;
  request_builder_ = std::move(request_builder);
  response_handler_ = std::move(response_handler);
  error_handler_ = std::move(error_handler);
  return true;
}

void RecoveryClient::start() {
  running_ = true;
  schedule(0);
}

void RecoveryClient::stop() {
  running_ = false;
  timer_->stop();
  if (reply_) {
    QNetworkReply *reply = std::exchange(reply_, nullptr);
    QObject::disconnect(reply, nullptr, timer_, nullptr);
    reply->abort();
    reply->deleteLater();
  }
}

void RecoveryClient::pause(bool paused) {
  paused_ = paused;
  if (paused_) {
    timer_->stop();
    if (reply_) {
      QNetworkReply *reply = std::exchange(reply_, nullptr);
      QObject::disconnect(reply, nullptr, timer_, nullptr);
      reply->abort();
      reply->deleteLater();
    }
  } else if (running_) {
    schedule(0);
  }
}

void RecoveryClient::poll() {
  if (!running_ || paused_ || reply_ || !request_builder_) return;
  const auto dispatch = request_builder_();
  if (!dispatch) {
    schedule(1000);
    return;
  }
  context_ = dispatch->context;
  QNetworkRequest request(QUrl(QString::fromStdString(config_.control_url)));
  request.setHeader(QNetworkRequest::ContentTypeHeader, "application/json");
  request.setRawHeader("Authorization",
                       QByteArray("Bearer ") + QByteArray::fromStdString(config_.controller_token));
  request.setAttribute(QNetworkRequest::RedirectPolicyAttribute,
                       QNetworkRequest::ManualRedirectPolicy);
  request.setAttribute(QNetworkRequest::CacheLoadControlAttribute,
                       QNetworkRequest::AlwaysNetwork);
  request.setTransferTimeout(5000);
  reply_ = network_->post(request, dispatch->body);
  QObject::connect(reply_, &QNetworkReply::finished, timer_, [this] { finished(); });
}

void RecoveryClient::finished() {
  QNetworkReply *reply = reply_;
  reply_ = nullptr;
  if (!reply) return;
  const QVariant redirect = reply->attribute(QNetworkRequest::RedirectionTargetAttribute);
  const int status = reply->attribute(QNetworkRequest::HttpStatusCodeAttribute).toInt();
  std::string failure;
  RecoveryPollResponse response;
  if (redirect.isValid()) failure = "recovery controller redirect rejected";
  else if (reply->error() != QNetworkReply::NoError || status != 200)
    failure = "recovery controller request failed";
  else if (parse_poll_response(reply->readAll(), response, failure) && response_handler_)
    response_handler_(context_, response);
  reply->deleteLater();
  if (!failure.empty() && error_handler_) error_handler_(failure);
  schedule(failure.empty() ? response.next_poll_ms : 1000);
}

void RecoveryClient::schedule(uint32_t milliseconds) {
  if (running_ && !paused_) timer_->start(static_cast<int>(milliseconds));
}

}
