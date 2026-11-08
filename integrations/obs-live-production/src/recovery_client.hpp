#pragma once

#include "config.hpp"
#include "recovery_protocol.hpp"

#include <QByteArray>

#include <cstdint>
#include <functional>
#include <optional>
#include <string>

class QNetworkAccessManager;
class QNetworkReply;
class QTimer;

namespace live_production {

struct PollDispatch {
  QByteArray body;
  PollContext context;
};

class RecoveryClient {
public:
  using RequestBuilder = std::function<std::optional<PollDispatch>()>;
  using ResponseHandler = std::function<void(const PollContext &,
                                             const RecoveryPollResponse &)>;
  using ErrorHandler = std::function<void(const std::string &)>;

  RecoveryClient();
  ~RecoveryClient();
  RecoveryClient(const RecoveryClient &) = delete;
  RecoveryClient &operator=(const RecoveryClient &) = delete;

  bool configure(const RecoveryConfig &config, RequestBuilder request_builder,
                 ResponseHandler response_handler, ErrorHandler error_handler,
                 std::string &error);
  void start();
  void stop();
  void pause(bool paused);
  bool in_flight() const { return reply_ != nullptr; }

private:
  void poll();
  void finished();
  void schedule(uint32_t milliseconds);

  RecoveryConfig config_;
  RequestBuilder request_builder_;
  ResponseHandler response_handler_;
  ErrorHandler error_handler_;
  QNetworkAccessManager *network_ = nullptr;
  QTimer *timer_ = nullptr;
  QNetworkReply *reply_ = nullptr;
  PollContext context_;
  bool running_ = false;
  bool paused_ = false;
};

}
