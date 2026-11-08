#include "recovery_client.hpp"

#include <QCoreApplication>
#include <QElapsedTimer>
#include <QEventLoop>
#include <QHostAddress>
#include <QTcpServer>
#include <QTcpSocket>

#include <iostream>
#include <functional>
#include <stdexcept>

namespace {

void require(bool value, const char *message) {
  if (!value) throw std::runtime_error(message);
}

bool wait_until(const std::function<bool()> &condition, int milliseconds) {
  QElapsedTimer timer;
  timer.start();
  while (!condition() && timer.elapsed() < milliseconds)
    QCoreApplication::processEvents(QEventLoop::AllEvents, 10);
  return condition();
}

live_production::RecoveryConfig config(quint16 port) {
  return {1, "http://127.0.0.1:" + std::to_string(port) +
                 "/api/v1/source-recovery/source/poll",
          "controller", "0123456789abcdef"};
}

live_production::PollDispatch dispatch() {
  return {QByteArray(R"json({"schemaVersion":1})json"), {1, 1, 1}};
}

void redirect_is_rejected_and_bearer_is_source_scoped() {
  QTcpServer server;
  require(server.listen(QHostAddress::LocalHost), "server listen failed");
  QByteArray request;
  QObject::connect(&server, &QTcpServer::newConnection, [&] {
    QTcpSocket *socket = server.nextPendingConnection();
    QObject::connect(socket, &QTcpSocket::readyRead, [&, socket] {
      request += socket->readAll();
      socket->write("HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1/other\r\nContent-Length: 0\r\n\r\n");
      socket->flush();
    });
  });
  live_production::RecoveryClient client;
  bool failed = false;
  bool responded = false;
  std::string error;
  require(client.configure(config(server.serverPort()), dispatch,
                           [&](const auto &, const auto &) { responded = true; },
                           [&](const std::string &) { failed = true; }, error),
          "client configuration failed");
  client.start();
  require(wait_until([&] { return failed; }, 2000), "redirect failure not observed");
  client.stop();
  require(!responded, "redirect produced a recovery response");
  require(request.contains("Authorization: Bearer 0123456789abcdef"),
          "source bearer header missing");
}

void only_one_request_is_in_flight() {
  QTcpServer server;
  require(server.listen(QHostAddress::LocalHost), "server listen failed");
  size_t connections = 0;
  QObject::connect(&server, &QTcpServer::newConnection, [&] {
    while (QTcpSocket *socket = server.nextPendingConnection()) {
      ++connections;
      socket->setParent(&server);
    }
  });
  live_production::RecoveryClient client;
  std::string error;
  require(client.configure(config(server.serverPort()), dispatch, [](const auto &, const auto &) {},
                           [](const std::string &) {}, error),
          "client configuration failed");
  client.start();
  require(wait_until([&] { return connections == 1; }, 1000),
          "first request was not opened");
  QElapsedTimer timer;
  timer.start();
  while (timer.elapsed() < 1200)
    QCoreApplication::processEvents(QEventLoop::AllEvents, 10);
  client.stop();
  require(connections == 1, "client opened more than one in-flight request");
}

}

int main(int argc, char **argv) {
  QCoreApplication application(argc, argv);
  try {
    redirect_is_rejected_and_bearer_is_source_scoped();
    only_one_request_is_in_flight();
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
