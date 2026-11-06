import assert from "node:assert/strict"
import { mkdir, mkdtemp, stat, writeFile } from "node:fs/promises"
import { join } from "node:path"
import test from "node:test"
import { ApiClient } from "./harness/api-client.mjs"
import { runSupervised } from "./harness/media-analysis.mjs"
import { OwnedProcesses, freeUdpPort, poll } from "./harness/processes.mjs"
import { startRecoveryFakeServer } from "./harness/recovery-fake-server.mjs"

const enabled = process.env.LIVE_PRODUCTION_RECOVERY_DRIVER === "1"

test("synthetic controller recovers real SRT AV without making an OBS claim", { skip: !enabled, timeout: 120_000 }, async () => {
  const root = requiredEnv("LIVE_PRODUCTION_RECOVERY_ROOT")
  const supervisor = requiredEnv("LIVE_PRODUCTION_RECOVERY_SUPERVISOR")
  await mkdir(join(root, "var"), { recursive: true, mode: 0o700 })
  const directory = await mkdtemp(join(root, "var/recovery-local-driver."))
  const processes = new OwnedProcesses(supervisor, { timeoutMs: 90_000, graceMs: 5_000 })
  const sourceId = "source-local-recovery"
  const controllerId = "11111111-1111-4111-8111-111111111111"
  const token = "local-source-controller-token"
  const fake = await startRecoveryFakeServer({ sourceId, controllerId, token, serverRunId: "server-initial" })
  const api = new ApiClient(fake.baseUrl)
  const port = await freeUdpPort()
  let mediaPath = join(directory, "received-0.ts")
  let sender = null; let receiver = null; let seq = 1; let leg = 0
  let aggregate = emptyAggregate()
  const evidence = []
  try {
    receiver = await startReceiver(processes, root, directory, port, mediaPath, leg)
    await api.pollRecovery(sourceId, token, request({ pollSeq: `${seq}` }))
    sender = await startSender(processes, root, directory, port, "initial")
    await waitForBytes(mediaPath, 150_000)
    let observed = cumulative(aggregate, probe(mediaPath, supervisor))
    fake.model().observeReceiver(observed, Date.now())
    await waitForBytes(mediaPath, observed.bytes + 80_000)
    observed = cumulative(aggregate, probe(mediaPath, supervisor)); fake.model().observeReceiver(observed, Date.now())
    seq += 1
    await api.pollRecovery(sourceId, token, request({ pollSeq: `${seq}`, intentEpoch: "2", localDesired: "running", publisherState: "sending" }))

    const freezeStart = observed
    processes.signalNative(sender, "SIGSTOP")
    await waitForStable(mediaPath, 3_100)
    observed = cumulative(aggregate, probe(mediaPath, supervisor)); fake.model().observeReceiver(observed, Date.now())
    seq += 1
    const freezeAction = (await api.pollRecovery(sourceId, token, request({ pollSeq: `${seq}`, intentEpoch: "2", localDesired: "running", publisherState: "failed" }))).action
    assert.equal(freezeAction.kind, "restartPublisher")
    await stopSenderGracefully(processes, sender, mediaPath, true); sender = null
    aggregate = addSegment(aggregate, await finalizeReceiver(processes, receiver, mediaPath, supervisor, false)); receiver = null
    leg += 1; mediaPath = join(directory, `received-${leg}.ts`)
    receiver = await startReceiver(processes, root, directory, port, mediaPath, leg)
    sender = await startSender(processes, root, directory, port, "after-freeze")
    seq += 1
    await api.pollRecovery(sourceId, token, request({ pollSeq: `${seq}`, intentEpoch: "2", localDesired: "running",
      publisherState: "sending", lastAction: { commandId: freezeAction.commandId, result: "started" } }))
    observed = await proveProgress(mediaPath, supervisor, fake, aggregate)
    evidence.push(caseEvidence("freeze", freezeStart, observed))

    processes.signalNative(sender, "SIGKILL"); await processes.wait(sender); sender = null
    await waitForStable(mediaPath, 3_100)
    const dropped = cumulative(aggregate, probe(mediaPath, supervisor)); fake.model().observeReceiver(dropped, Date.now())
    seq += 1
    const dropAction = (await api.pollRecovery(sourceId, token, request({ pollSeq: `${seq}`, intentEpoch: "2", localDesired: "running", publisherState: "failed" }))).action
    fake.restart("server-restarted")
    const replay = (await api.pollRecovery(sourceId, token, request({ pollSeq: `${seq}`, intentEpoch: "2", localDesired: "running", publisherState: "failed" }))).action
    assert.equal(replay.commandId, dropAction.commandId); assert.equal(replay.serverRunId, "server-restarted")
    aggregate = addSegment(aggregate, await finalizeReceiver(processes, receiver, mediaPath, supervisor, false)); receiver = null
    leg += 1; mediaPath = join(directory, `received-${leg}.ts`)
    receiver = await startReceiver(processes, root, directory, port, mediaPath, leg)
    sender = await startSender(processes, root, directory, port, "after-drop")
    seq += 1
    await api.pollRecovery(sourceId, token, request({ pollSeq: `${seq}`, intentEpoch: "2", localDesired: "running",
      publisherState: "sending", lastAction: { commandId: replay.commandId, result: "started" } }))
    observed = await proveProgress(mediaPath, supervisor, fake, aggregate)
    evidence.push(caseEvidence("drop-and-server-restart", dropped, observed))

    processes.signalNative(sender, "SIGSTOP"); await waitForStable(mediaPath, 3_100)
    const stoppedAt = cumulative(aggregate, probe(mediaPath, supervisor)); fake.model().observeReceiver(stoppedAt, Date.now())
    fake.model().setPolicy({ maintenance: true })
    seq += 1
    assert.equal((await api.pollRecovery(sourceId, token, request({ pollSeq: `${seq}`, intentEpoch: "2", localDesired: "running" }))).action, null)
    fake.model().setPolicy({ maintenance: false, sourceEnabled: false })
    seq += 1
    assert.equal((await api.pollRecovery(sourceId, token, request({ pollSeq: `${seq}`, intentEpoch: "2", localDesired: "running" }))).action, null)
    fake.model().setPolicy({ sourceEnabled: true })
    seq += 1
    const queued = (await api.pollRecovery(sourceId, token, request({ pollSeq: `${seq}`, intentEpoch: "2", localDesired: "running" }))).action
    assert.equal(queued.kind, "restartPublisher")
    seq += 1
    assert.equal((await api.pollRecovery(sourceId, token, request({ pollSeq: `${seq}`, intentEpoch: "2", localDesired: "stopped" }))).action, null)
    await stopSenderGracefully(processes, sender, mediaPath, true); sender = null
    evidence.push({ kind: "manual-stop-fences-queued-restart", bytesBefore: stoppedAt.bytes, restarted: false })

    assert.equal((await rawPoll(fake.baseUrl, sourceId, null)).status, 401)
    assert.equal((await rawPoll(fake.baseUrl, sourceId, "wrong-token")).status, 401)
    assert.equal((await rawPoll(fake.baseUrl, "source-other", token)).status, 401)
    aggregate = addSegment(aggregate, await finalizeReceiver(processes, receiver, mediaPath, supervisor, false)); receiver = null
    const final = aggregate
    assert(final.videoFrames > 30 && final.audioBuffers > 30)
    const strictCalibration = await proveStrictCalibration(processes, root, directory, supervisor)
    assert.equal(fake.requests.every((entry) => entry.cookiePresent === false && entry.operatorHeaderPresent === false), true)
    const report = { schemaVersion: 1, status: "pass", claim: "synthetic-gstreamer-controller-not-obs",
      receiver: final, receiverLegs: leg + 1, strictCalibration,
      reconnectLimitation: "faulted live legs use decoded counters; separate finite leg proves full strict codec decode",
      cases: evidence, controllerRequests: fake.requests.length,
      genericMutationRequests: fake.requests.filter((entry) => entry.url.includes("/commands") || entry.url.includes("/outputs")).length }
    assert.equal(report.genericMutationRequests, 0)
    await writeFile(join(directory, "recovery-local-report.json"), `${JSON.stringify(report, null, 2)}\n`, { mode: 0o600 })
  } finally {
    if (sender !== null) await processes.stop(sender)
    if (receiver !== null) await processes.stop(receiver)
    await processes.stopAll(); await fake.close()
  }
})

function request(overrides) {
  return { schemaVersion: 1, controllerId: "11111111-1111-4111-8111-111111111111",
    processEpoch: "1", pollSeq: "1", intentEpoch: "1", localDesired: "stopped",
    publisherState: "stopped", lastAction: null, ...overrides }
}

async function startReceiver(processes, root, directory, port, mediaPath, leg) {
  return processes.spawn("recovery-receiver", "gst-launch-1.0", ["-q", "-e", "srtsrc",
    `uri=srt://127.0.0.1:${port}?mode=listener`, "wait-for-connection=true", "keep-listening=false",
    "!", "filesink", `location=${mediaPath}`], { cwd: root, env: process.env, logPath: join(directory, `receiver-${leg}.log`) })
}

async function startSender(processes, root, directory, port, label) {
  return processes.spawn(`recovery-sender-${label}`, "gst-launch-1.0", ["-q", "-e", "mpegtsmux", "name=mux", "alignment=7",
    "!", "srtsink", `uri=srt://127.0.0.1:${port}?mode=caller`, "wait-for-connection=true", "auto-reconnect=false",
    "videotestsrc", "is-live=true", "pattern=ball", "!", "video/x-raw,width=640,height=360,framerate=30/1", "!",
    "videoconvert", "!", "video/x-raw,format=I420", "!", "x264enc", "tune=zerolatency", "key-int-max=30", "bitrate=1000",
    "byte-stream=true", "option-string=repeat-headers=1", "!", "h264parse", "config-interval=-1", "!", "mux.",
    "audiotestsrc", "is-live=true", "wave=sine", "freq=440", "!", "audio/x-raw,rate=48000,channels=2", "!",
    "voaacenc", "bitrate=128000", "!", "aacparse", "!", "mux."],
  { cwd: root, env: process.env, logPath: join(directory, `${label}-sender.log`) })
}

async function startFiniteSender(processes, root, directory, port) {
  return processes.spawn("recovery-finite-sender", "gst-launch-1.0", ["-q", "-e", "mpegtsmux", "name=mux", "alignment=7",
    "!", "tee", "name=drain", "drain.", "!", "queue", "!", "srtsink", `uri=srt://127.0.0.1:${port}?mode=caller`, "wait-for-connection=true", "auto-reconnect=false",
    "drain.", "!", "queue", "!", "filesink", `location=${join(directory, "strict-source.ts")}`,
    "drain.", "!", "queue", "!", "fakesink", "sync=true", "ts-offset=500000000",
    "videotestsrc", "is-live=true", "num-buffers=150", "pattern=ball", "!", "video/x-raw,width=640,height=360,framerate=30/1", "!",
    "videoconvert", "!", "video/x-raw,format=I420", "!", "x264enc", "tune=zerolatency", "key-int-max=30", "bitrate=1000",
    "byte-stream=true", "option-string=repeat-headers=1", "!", "h264parse", "config-interval=-1", "!", "mux.",
    "audiotestsrc", "is-live=true", "num-buffers=235", "wave=sine", "freq=440", "!", "audio/x-raw,rate=48000,channels=2", "!",
    "voaacenc", "bitrate=128000", "!", "aacparse", "!", "mux."],
  { cwd: root, env: process.env, logPath: join(directory, "finite-sender.log") })
}

async function waitForBytes(path, minimum) {
  await poll(() => stat(path).then((value) => value.size >= minimum).catch(() => false), 15_000, `${minimum} receiver bytes`)
}

async function waitForStable(path, stableMs) {
  let size = -1; let changedAt = Date.now()
  await poll(async () => { const current = (await stat(path)).size
    if (current !== size) { size = current; changedAt = Date.now() }
    return Date.now() - changedAt >= stableMs }, stableMs + 10_000, "receiver byte stability")
}

async function proveProgress(path, supervisor, fake, aggregate) {
  await waitForBytes(path, 100_000)
  let local = probe(path, supervisor); let observed = cumulative(aggregate, local)
  fake.model().observeReceiver(observed, Date.now())
  await waitForBytes(path, local.bytes + 80_000)
  local = probe(path, supervisor); observed = cumulative(aggregate, local)
  fake.model().observeReceiver(observed, Date.now())
  return observed
}

async function finalizeReceiver(processes, receiver, path, supervisor, strict) {
  await processes.wait(receiver, 5_000).catch(() => processes.stop(receiver))
  return probe(path, supervisor, strict)
}

async function proveStrictCalibration(processes, root, directory, supervisor) {
  const port = await freeUdpPort(); const path = join(directory, "strict-calibration.ts")
  const receiver = await startReceiver(processes, root, directory, port, path, "calibration")
  const sender = await startFiniteSender(processes, root, directory, port)
  await processes.wait(sender, 15_000); await processes.wait(receiver, 15_000)
  const evidence = probe(path, supervisor, true)
  const source = probe(join(directory, "strict-source.ts"), supervisor, true)
  assert(evidence.videoFrames >= 140 && evidence.audioBuffers > 100)
  assert.equal(evidence.videoFrames, 150)
  assert.equal(evidence.videoFrames, source.videoFrames)
  assert.equal(evidence.audioBuffers, source.audioBuffers)
  return { ...evidence, sourceFrameCounts: { video: source.videoFrames, audio: source.audioBuffers } }
}

async function stopSenderGracefully(processes, sender, mediaPath, resume) {
  if (resume) {
    const before = (await stat(mediaPath)).size
    processes.signalNative(sender, "SIGCONT")
    await waitForBytes(mediaPath, before + 80_000)
  }
  processes.signalNative(sender, "SIGINT")
  await processes.wait(sender)
}

function probe(path, supervisor, strict = false) {
  const output = JSON.parse(runSupervised(supervisor, "ffprobe", ["-v", "error", "-count_frames", "-show_streams", "-show_format", "-of", "json", path]))
  const video = output.streams.find((stream) => stream.codec_type === "video")
  const audio = output.streams.find((stream) => stream.codec_type === "audio")
  if (video === undefined || audio === undefined) throw new Error("Recovery receiver omitted video or audio")
  if (strict) runSupervised(supervisor, "ffmpeg", ["-v", "error", "-xerror", "-i", path, "-f", "null", "-"])
  const videoFrames = Number.parseInt(video.nb_read_frames ?? "0", 10)
  const audioBuffers = Number.parseInt(audio.nb_read_frames ?? "0", 10)
  return { bytes: Number.parseInt(output.format.size, 10), videoFrames, audioBuffers, strictDecoded: strict,
    lastVideoPtsNs: videoFrames * 33_333_333, continuityGeneration: "1",
    video: { codec: video.codec_name, width: video.width, height: video.height },
    audio: { codec: audio.codec_name, sampleRate: Number.parseInt(audio.sample_rate, 10), channels: audio.channels } }
}

function caseEvidence(kind, before, after) {
  return { kind, videoFramesBefore: before.videoFrames, videoFramesAfter: after.videoFrames,
    audioBuffersBefore: before.audioBuffers, audioBuffersAfter: after.audioBuffers,
    usableAvRecovered: after.videoFrames > before.videoFrames && after.audioBuffers > before.audioBuffers }
}

function emptyAggregate() {
  return { bytes: 0, videoFrames: 0, audioBuffers: 0, lastVideoPtsNs: 0, continuityGeneration: "1",
    video: { codec: "h264", width: 640, height: 360 }, audio: { codec: "aac", sampleRate: 48_000, channels: 2 }, segments: [] }
}

function addSegment(aggregate, segment) {
  return { ...aggregate, bytes: aggregate.bytes + segment.bytes,
    videoFrames: aggregate.videoFrames + segment.videoFrames,
    audioBuffers: aggregate.audioBuffers + segment.audioBuffers,
    lastVideoPtsNs: (aggregate.videoFrames + segment.videoFrames) * 33_333_333,
    segments: [...aggregate.segments, { bytes: segment.bytes, videoFrames: segment.videoFrames,
      audioBuffers: segment.audioBuffers, strictDecoded: segment.strictDecoded }] }
}

function cumulative(aggregate, segment) {
  return { ...segment, bytes: aggregate.bytes + segment.bytes,
    videoFrames: aggregate.videoFrames + segment.videoFrames,
    audioBuffers: aggregate.audioBuffers + segment.audioBuffers,
    lastVideoPtsNs: (aggregate.videoFrames + segment.videoFrames) * 33_333_333 }
}

async function rawPoll(baseUrl, sourceId, token) {
  return fetch(`${baseUrl}/source-recovery/${sourceId}/poll`, { method: "POST",
    headers: { "content-type": "application/json", ...(token === null ? {} : { authorization: `Bearer ${token}` }) },
    body: JSON.stringify(request({})) })
}

function requiredEnv(name) { const value = process.env[name]; if (value === undefined) throw new Error(`${name} is required`); return value }
