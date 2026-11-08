#include "recovery_protocol.hpp"

#include <QByteArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonParseError>
#include <QString>

#include <utility>

namespace live_production {
namespace {

bool decimal(const QJsonObject &object, const char *key, uint64_t &value) {
  const QString text = object.value(key).toString();
  if (text.isEmpty()) return false;
  bool ok = false;
  value = text.toULongLong(&ok);
  return ok;
}

bool required_string(const QJsonObject &object, const char *key,
                     std::string &value) {
  const QJsonValue field = object.value(key);
  if (!field.isString() || field.toString().isEmpty()) return false;
  value = field.toString().toStdString();
  return true;
}

}

bool parse_poll_response(const QByteArray &bytes, RecoveryPollResponse &response,
                         std::string &error) {
  QJsonParseError parse_error;
  const QJsonDocument document = QJsonDocument::fromJson(bytes, &parse_error);
  if (parse_error.error != QJsonParseError::NoError || !document.isObject()) {
    error = "recovery response is not valid JSON";
    return false;
  }
  const QJsonObject root = document.object();
  const QJsonObject policy = root.value("policy").toObject();
  const QJsonObject controller = root.value("controller").toObject();
  const QJsonObject receiver = root.value("receiver").toObject();
  const QJsonObject retry = root.value("retry").toObject();
  response.fence.policy_enabled = policy.value("enabled").toBool(false);
  response.fence.maintenance = policy.value("maintenance").toBool(false);
  response.fence.source_enabled = root.value("sourceEnabled").toBool(false);
  response.controller_online = controller.value("online").toBool(false);
  response.controller_local_desired =
      controller.value("localDesired").toString("stopped").toStdString();
  response.receiver_state = receiver.value("state").toString("unknown").toStdString();
  response.retry_attempts = static_cast<uint32_t>(retry.value("attempts").toInt());
  response.retry_result = retry.value("lastResult").toString("never").toStdString();
  if (root.value("schemaVersion").toInt() != 1 ||
      !required_string(root, "sourceId", response.fence.source_id) ||
      !required_string(root, "serverRunId", response.fence.server_run_id) ||
      !required_string(controller, "controllerId", response.fence.controller_id) ||
      !decimal(root, "processEpoch", response.echoed.process_epoch) ||
      !decimal(root, "pollSeq", response.echoed.poll_seq) ||
      !decimal(root, "intentEpoch", response.echoed.intent_epoch)) {
    error = "recovery response omitted required fences";
    return false;
  }
  response.fence.process_epoch = response.echoed.process_epoch;
  response.fence.poll_seq = response.echoed.poll_seq;
  response.fence.intent_epoch = response.echoed.intent_epoch;
  const int next_poll = root.value("nextPollMs").toInt(1000);
  if (next_poll < 100 || next_poll > 60'000) {
    error = "recovery response has invalid poll interval";
    return false;
  }
  response.next_poll_ms = static_cast<uint32_t>(next_poll);
  response.action.reset();
  if (!root.value("action").isNull()) {
    const QJsonObject value = root.value("action").toObject();
    RecoveryAction action;
    int attempt = value.value("attempt").toInt();
    if (!required_string(value, "kind", action.kind) ||
        !required_string(value, "commandId", action.command_id) ||
        !required_string(value, "serverRunId", action.server_run_id) ||
        !required_string(value, "controllerId", action.controller_id) ||
        !decimal(value, "processEpoch", action.process_epoch) ||
        !decimal(value, "intentEpoch", action.intent_epoch) || attempt < 1 ||
        attempt > 3) {
      error = "recovery action omitted required fences";
      return false;
    }
    action.attempt = static_cast<uint32_t>(attempt);
    response.action = std::move(action);
  }
  return true;
}

}
