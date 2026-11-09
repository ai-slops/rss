#include "all_sources.hpp"
#include "config.hpp"
#include <obs-module.h>
#include <QCryptographicHash>
#include <QCoreApplication>
#include <QFileInfo>
#include <QEventLoop>
#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QNetworkReply>
#include <QNetworkRequest>
#include <QSettings>
#include <QRegularExpression>
#include <QStringList>
#include <QSslCertificate>
#include <QSslConfiguration>
#include <QUuid>
#include <QVariant>
#include <algorithm>
#include <cmath>
#include <set>
#include <stdexcept>

namespace live_production {
namespace {
struct Source { QString uuid, name; uint32_t width, height; bool audio, video; };
void append_source(std::vector<Source> &sources, obs_source_t *source) {
  if (!source || obs_source_get_type(source) != OBS_SOURCE_TYPE_INPUT) return;
  const QString uuid = QString::fromUtf8(obs_source_get_uuid(source));
  if (uuid.isEmpty() || std::any_of(sources.begin(), sources.end(), [&](const Source &s) { return s.uuid == uuid; })) return;
  const uint32_t flags = obs_source_get_output_flags(source);
  if (!(flags & (OBS_SOURCE_VIDEO | OBS_SOURCE_AUDIO))) return;
  sources.push_back({uuid, QString::fromUtf8(obs_source_get_name(source)),
                    obs_source_get_width(source), obs_source_get_height(source), bool(flags & OBS_SOURCE_AUDIO), bool(flags & OBS_SOURCE_VIDEO)});
}
std::vector<Source> enumerate_sources() {
  std::vector<Source> sources;
  obs_enum_sources([](void *context, obs_source_t *source) {
    append_source(*static_cast<std::vector<Source> *>(context), source); return true;
  }, &sources);
  for (uint32_t channel = 1; channel < MAX_CHANNELS; ++channel) {
    obs_source_t *source = obs_get_output_source(channel);
    append_source(sources, source);
    if (source) obs_source_release(source);
  }
  return sources;
}
QString string(const QJsonObject &object, const char *key) { return object.value(key).toString(); }
}
AllSourcesPublisher::AllSourcesPublisher() {
  QCoreApplication::addLibraryPath(QFileInfo(QString::fromUtf8(obs_get_module_binary_path(obs_current_module()))).absolutePath());
  timer_.setInterval(1000);
  QObject::connect(&timer_, &QTimer::timeout, &timer_, [this] { poll(); });
}
AllSourcesPublisher::~AllSourcesPublisher() { stop(); }
void AllSourcesPublisher::stop() {
  cancelled_ = true;
  running_ = false;
  timer_.stop();
  inputs_.clear();
  message_ = "Individual source publishing stopped";
}
QJsonObject AllSourcesPublisher::request(const char *method, const QString &path, const QJsonObject &body) {
  if (cancelled_) throw std::runtime_error("Individual source publishing cancelled");
  QUrl url(server_); url.setPath("/api/v1" + path);
  QNetworkRequest request(url);
  request.setRawHeader("x-live-production", "1");
  request.setHeader(QNetworkRequest::ContentTypeHeader, "application/json");
  request.setAttribute(QNetworkRequest::RedirectPolicyAttribute, QNetworkRequest::ManualRedirectPolicy);
  request.setTransferTimeout(8000);
  QNetworkReply *reply = QByteArray(method) == "GET" ? network_.get(request) : network_.post(request, QJsonDocument(body).toJson(QJsonDocument::Compact));
  QEventLoop loop;
  QObject::connect(reply, &QNetworkReply::finished, &loop, &QEventLoop::quit);
  loop.exec();
  const int code = reply->attribute(QNetworkRequest::HttpStatusCodeAttribute).toInt();
  const bool ok = reply->error() == QNetworkReply::NoError && code >= 200 && code < 300;
  const QByteArray bytes = reply->readAll();
  const QString network_error = reply->errorString();
  reply->deleteLater();
  if (!ok) throw std::runtime_error((QString("Server request failed (%1): %2").arg(code).arg(network_error)).toStdString());
  if (cancelled_) throw std::runtime_error("Individual source publishing cancelled");
  return bytes.isEmpty() ? QJsonObject{} : QJsonDocument::fromJson(bytes).object();
}
QJsonObject AllSourcesPublisher::snapshot() { return request("GET", "/snapshot"); }
void AllSourcesPublisher::command(const QString &kind, const QString &target, const QJsonObject &payload, const QString &revision) {
  const auto current = snapshot();
  QJsonObject body {{"id", QUuid::createUuid().toString(QUuid::WithoutBraces)}, {"kind", kind}, {"targetId", target}, {"expectedEngineRunId", current.value("engineRunId")}, {"payload", payload}};
  if (!revision.isEmpty()) body.insert("expectedRevision", revision);
  const auto receipt = request("POST", "/commands", body);
  if (string(receipt, "status") != "applied") throw std::runtime_error("Server rejected source or layout configuration; refresh and retry");
}
bool AllSourcesPublisher::start(const QString &connection_file, QString &error) {
  if (starting_) { error = "Source registration is already running"; return false; }
  stop(); cancelled_ = false; starting_ = true;
  try {
    std::string private_error;
    if (!is_private_regular_file(connection_file.toStdString(), private_error)) throw std::runtime_error(private_error);
    QFile file(connection_file);
    if (!file.open(QIODevice::ReadOnly) || file.size() > 1024 * 1024) throw std::runtime_error("Cannot read private connection file");
    auto config = QJsonDocument::fromJson(file.readAll()).object(); file.close();
    server_ = QUrl(string(config, "serverUrl"));
    if (config.value("schemaVersion").toInt() != 1 || server_.scheme() != "https" || server_.host().isEmpty() || !server_.userInfo().isEmpty() || server_.hasQuery() || server_.hasFragment()) throw std::runtime_error("Connection file requires a valid HTTPS server URL");
    source_host_ = string(config, "sourceHost");
    if (source_host_.isEmpty() || source_host_.contains(QRegularExpression("[/?&#\\s]"))) throw std::runtime_error("Invalid source host");
    const auto certificates = QSslCertificate::fromData(string(config, "caCertificatePem").toUtf8());
    if (!certificates.isEmpty()) {
      auto ssl = QSslConfiguration::defaultConfiguration();
      auto authorities = ssl.caCertificates(); authorities.append(certificates); ssl.setCaCertificates(authorities);
      QSslConfiguration::setDefaultConfiguration(ssl);
    }
    const auto sources = enumerate_sources();
    if (sources.empty()) throw std::runtime_error("No individual OBS input sources were found");
    request("POST", "/auth/login", {{"password", config.take("password")}});
    auto current = snapshot(); auto project = current.value("project").toObject();
    QSettings bindings("LiveProduction", "IndividualSources");
    const QString prefix = QString::fromLatin1(QCryptographicHash::hash(server_.toString().toUtf8(), QCryptographicHash::Sha256).toHex()) + "/";
    std::set<int> ports;
    for (const auto &entry : project.value("sources").toArray()) { const auto adapter = entry.toObject().value("adapter").toObject(); if (string(adapter, "kind") == "srt") ports.insert(adapter.value("port").toInt()); }
    size_t missing = 0;
    const auto bound_id = [&](const Source &source) { return bindings.value(prefix + source.uuid).toString(); };
    const auto find_source = [&](const QString &id) { for (const auto &entry : project.value("sources").toArray()) if (entry.toObject().value("id").toString() == id) return entry.toObject(); return QJsonObject{}; };
    for (const auto &source : sources) if (find_source(bound_id(source)).isEmpty()) ++missing;
    size_t available = 0; for (int port = 9000; port <= 9015; ++port) if (!ports.count(port)) ++available;
    if (missing > available) throw std::runtime_error("Not enough forwarded source ports (9000-9015); remove unused inputs or extend the forwarded port range");
    std::vector<std::pair<Source, QJsonObject>> registered;
    for (const auto &source : sources) {
      QJsonObject remote = find_source(bound_id(source));
      if (remote.isEmpty()) {
        int port = 9000; while (ports.count(port)) ++port; ports.insert(port);
        command("source.create", string(project, "id"), {{"name", source.name}, {"adapter", QJsonObject{{"kind", "srt"}, {"port", port}, {"bufferMs", 250}}}});
        const auto updated = snapshot().value("project").toObject();
        const auto previous = project.value("sources").toArray();
        for (const auto &entry : updated.value("sources").toArray()) {
          bool exists = false; for (const auto &old : previous) if (old.toObject().value("id") == entry.toObject().value("id")) exists = true;
          if (!exists) remote = entry.toObject();
        }
        project = updated;
        if (remote.isEmpty()) throw std::runtime_error("Created source could not be identified");
        bindings.setValue(prefix + source.uuid, string(remote, "id")); bindings.sync();
      }
      registered.emplace_back(source, remote);
      const auto endpoint = request("GET", "/sources/" + string(remote, "id") + "/publishing-endpoint");
      PublisherConfig publisher; publisher.schema_version = 1;
      publisher.target.source_id = string(remote, "id").toStdString();
      for (const auto &track : remote.value("tracks").toArray()) { const auto t = track.toObject(); if (string(t, "kind") == "video") publisher.target.video_track_id = string(t, "id").toStdString(); else publisher.target.audio_track_id = string(t, "id").toStdString(); }
      publisher.target.host = source_host_.toStdString(); publisher.target.port = static_cast<uint16_t>(endpoint.value("port").toInt());
      publisher.target.passphrase = string(endpoint, "passphrase").toStdString(); publisher.target.stream_id = string(endpoint, "streamId").toStdString(); publisher.target.buffer_ms = static_cast<uint32_t>(endpoint.value("bufferMs").toInt());
      publisher.obs.source_name = source.name.toStdString(); publisher.obs.source_uuid = source.uuid.toStdString(); publisher.obs.source_audio = true;
      const bool video = source.video;
      const uint32_t native_width = source.width ? source.width : 1920;
      const uint32_t native_height = source.height ? source.height : 1080;
      double scale = video ? std::min({1.0, 1920.0 / native_width, 1080.0 / native_height}) : 1.0;
      publisher.encoding.width = video ? std::max(16u, uint32_t(native_width * scale) / 2 * 2) : 16;
      publisher.encoding.height = video ? std::max(16u, uint32_t(native_height * scale) / 2 * 2) : 16;
      publisher.encoding.fps_numerator = video ? 60 : 1; publisher.encoding.fps_denominator = 1;
      publisher.encoding.video_bitrate_kbps = video ? 8000 : 64; publisher.encoding.audio_sample_rate = 48000;
      auto instance = std::make_unique<Publisher>(publisher, "individual-" + publisher.target.source_id, [] {});
      std::string failure;
      if (!instance->initialize(failure)) throw std::runtime_error((source.name + ": " + QString::fromStdString(failure)).toStdString());
      inputs_.push_back({std::move(instance), source.name, std::chrono::steady_clock::now()});
    }
    // One initial web-editable layout. Later starts preserve user layout changes.
    project = snapshot().value("project").toObject();
    QJsonObject scene; for (const auto &entry : project.value("scenes").toArray()) if (string(entry.toObject(), "name") == "OBS individual sources") scene = entry.toObject();
    if (scene.isEmpty()) {
      command("scene.create", string(project, "id"), {{"name", "OBS individual sources"}});
      project = snapshot().value("project").toObject();
      for (const auto &entry : project.value("scenes").toArray()) if (string(entry.toObject(), "name") == "OBS individual sources") scene = entry.toObject();
      QJsonArray items;
      size_t count = 0; for (const auto &entry : registered) if (entry.first.video) ++count;
      const int columns = std::max(1, int(std::ceil(std::sqrt(double(count))))); const int rows = std::max(1, int((count + columns - 1) / columns));
      const auto canvas = project.value("canvas").toObject(); const double width = canvas.value("width").toDouble() / columns, height = canvas.value("height").toDouble() / rows;
      size_t index = 0;
      for (const auto &entry : registered) {
        if (!entry.first.video) continue;
        QString track; for (const auto &value : entry.second.value("tracks").toArray()) if (string(value.toObject(), "kind") == "video") track = string(value.toObject(), "id");
        items.append(QJsonObject{{"id", QUuid::createUuid().toString(QUuid::WithoutBraces)}, {"name", entry.first.name}, {"visible", true}, {"locked", false}, {"rect", QJsonObject{{"x", double(index % columns) * width}, {"y", double(index / columns) * height}, {"width", width}, {"height", height}}}, {"crop", QJsonObject{{"left", 0}, {"top", 0}, {"right", 0}, {"bottom", 0}}}, {"fit", "contain"}, {"opacity", 1.0}, {"content", QJsonObject{{"kind", "track"}, {"trackId", track}}}}); ++index;
      }
      scene.insert("items", items); command("scene.save", string(scene, "id"), {{"scene", scene}}, string(scene, "revision"));
      project = snapshot().value("project").toObject();
      command("scene.move", string(scene, "id"), {{"index", 0}}, string(project, "revision"));
      project = snapshot().value("project").toObject(); for (const auto &entry : project.value("scenes").toArray()) if (string(entry.toObject(), "id") == string(scene, "id")) scene = entry.toObject();
      QJsonArray routes;
      for (const auto &entry : registered) if (entry.first.audio) for (const auto &value : entry.second.value("tracks").toArray()) if (string(value.toObject(), "kind") == "audio") routes.append(QJsonObject{{"trackId", value.toObject().value("id")}, {"gainDb", 0}, {"muted", false}, {"routed", true}});
      const auto before_audio = snapshot();
      bool sending = false;
      for (const auto &output : project.value("outputs").toArray()) {
        if (string(output.toObject().value("adapter").toObject(), "kind") != "rtmps") continue;
        for (const auto &status : before_audio.value("outputStatuses").toArray())
          if (status.toObject().value("outputId") == output.toObject().value("id") && string(status.toObject(), "desired") == "running") sending = true;
      }
      if (!sending) command("audio.apply", string(project, "id"), {{"routes", routes}}, string(project, "revision"));
      command("scene.cue", string(scene, "id"), {}, string(scene, "revision"));
      // The operator applies this prepared layout with Take in the web studio.
    }
    for (auto &input : inputs_) { std::string failure; if (!input.publisher->start(failure)) throw std::runtime_error((input.name + ": " + QString::fromStdString(failure)).toStdString()); }
    starting_ = false; running_ = true; message_ = ""; timer_.start(); return true;
  } catch (const std::exception &failure) { stop(); starting_ = false; error = QString::fromUtf8(failure.what()); message_ = error; return false; }
}
void AllSourcesPublisher::poll() {
  if (!running_ || starting_) return;
  for (auto &input : inputs_) {
    if (input.publisher->state() == "failed" && std::chrono::steady_clock::now() >= input.retry_at) {
      const PublisherConfig config = input.publisher->config();
      input.publisher.reset(); input.publisher = std::make_unique<Publisher>(config, "individual-" + config.target.source_id, [] {});
      std::string error; if (input.publisher->initialize(error)) input.publisher->start(error);
      input.retry_at = std::chrono::steady_clock::now() + std::chrono::seconds(5);
    }
  }
}
QString AllSourcesPublisher::status() const {
  if (starting_) return "Registering individual OBS sources…";
  if (!running_) return message_;
  QStringList lines;
  for (const auto &input : inputs_) lines.append(input.name + ": " + QString::fromStdString(input.publisher->state()));
  return lines.join("\n");
}
}
