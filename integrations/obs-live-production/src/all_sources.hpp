#pragma once
#include "publisher.hpp"
#include <QJsonObject>
#include <QNetworkAccessManager>
#include <QTimer>
#include <QUrl>
#include <QString>
#include <chrono>
#include <memory>
#include <vector>
namespace live_production {
class AllSourcesPublisher {
public:
  AllSourcesPublisher();
  ~AllSourcesPublisher();
  bool start(const QString &connection_file, QString &error);
  void stop();
  QString status() const;
  bool active() const { return starting_ || running_; }
private:
  struct Input { std::unique_ptr<Publisher> publisher; QString name; std::chrono::steady_clock::time_point retry_at; };
  QJsonObject request(const char *method, const QString &path, const QJsonObject &body = {});
  QJsonObject snapshot();
  void command(const QString &kind, const QString &target, const QJsonObject &payload, const QString &revision = {});
  void poll();
  QNetworkAccessManager network_;
  QTimer timer_;
  QUrl server_;
  QString source_host_;
  std::vector<Input> inputs_;
  bool starting_ = false;
  bool running_ = false;
  bool cancelled_ = false;
  QString message_;
};
}
