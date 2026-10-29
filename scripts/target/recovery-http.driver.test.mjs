import assert from "node:assert/strict"
import { mkdir, mkdtemp, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { randomBytes } from "node:crypto"
import test from "node:test"
import { startServer } from "./harness/native-runtime.mjs"
import { OwnedProcesses, freeUdpPort, poll } from "./harness/processes.mjs"
import { captureStrictSourceRecording, createProgressTracker, updateRecoveryPolicy, waitForQuiescentMedia } from "./harness/recovery-evidence.mjs"
import { UdpSrtBridge } from "./harness/udp-srt-bridge.mjs"

const enabled = process.env.LIVE_PRODUCTION_RECOVERY_HTTP_DRIVER === "1"

test("real server recovery API restarts synthetic SRT media without output or Program mutation", { skip: !enabled, timeout: 120_000 }, async () => {
  const root = env("LIVE_PRODUCTION_RECOVERY_ROOT")
  const supervisorBinary = env("LIVE_PRODUCTION_RECOVERY_SUPERVISOR")
  const options = { root, supervisorBinary, serverBinary: env("LIVE_PRODUCTION_RECOVERY_SERVER"),
    webDist: env("LIVE_PRODUCTION_RECOVERY_WEB_DIST"), deadlineSeconds: 90 }
  await mkdir(join(root, "var"), { recursive: true, mode: 0o700 })
  const directory = await mkdtemp(join(root, "var/recovery-http-driver."))
  const processes = new OwnedProcesses(supervisorBinary, { timeoutMs: 90_000, graceMs: 5_000 })
  const server = await startServer(processes, options, directory, randomBytes(32).toString("hex"), "recovery")
  let sender = null; let bridge = null; const sequence = { value: 1 }
  try {
    const before = await server.api.snapshot()
    const port = await freeUdpPort()
    await server.api.command("source.create", before.project.id, {
      name: "Recovery Synthetic", adapter: { kind: "srt", bindAddress: "127.0.0.1", port, bufferMs: 120 },
    }, { snapshot: before })
    let snapshot = await server.api.snapshot()
    const source = snapshot.project.sources.find((candidate) => candidate.name === "Recovery Synthetic")
    assert(source !== undefined)
    const endpoint = await server.api.publishingEndpoint(source.id)
    bridge = new UdpSrtBridge({ serverPort: endpoint.port })
    const bridgePort = await bridge.start()
    const bridgedEndpoint = { ...endpoint, port: bridgePort }
    const policy = await updateRecoveryPolicy(server.api, source.id, { enabled: true, maintenance: false })
    assert(BigInt(policy.after.policyVersion) >= 2n)
    const enrollment = await server.api.enrollRecovery(source.id, false)
    assert.equal(enrollment.policyVersion, policy.after.policyVersion)
    await server.api.pollRecovery(source.id, enrollment.controllerToken, request(enrollment.controllerId, sequence.value))
    sender = await startSender(processes, root, directory, bridgedEndpoint, "initial")
    await waitConnected(server.api, source.id)
    sequence.value += 1
    await server.api.pollRecovery(source.id, enrollment.controllerToken,
      request(enrollment.controllerId, sequence.value, { intentEpoch: "2", localDesired: "running", publisherState: "sending" }))
    const healthy = await waitRecovery(server.api, source.id, (status) => numeric(status.receiver.videoFrames) > 20
      && numeric(status.receiver.audioBuffers) > 20 && status.receiver.state === "healthy")

    processes.signalNative(sender, "SIGSTOP")
    const freeze = await waitAction(server.api, source.id, enrollment, sequence)
    assert.equal(freeze.action.kind, "restartPublisher")
    const handoverPromise = bridge.authorizeHandover()
    await stopSenderGracefully(processes, sender); sender = null
    await waitPublisherReady(server.api, source.id)
    await new Promise((resolve) => setTimeout(resolve, 3_000))
    sender = await startSender(processes, root, directory, bridgedEndpoint, "recreated")
    const handover = await handoverPromise
    sequence.value += 1
    await server.api.pollRecovery(source.id, enrollment.controllerToken, request(enrollment.controllerId, sequence.value, {
      intentEpoch: "2", localDesired: "running", publisherState: "sending",
      lastAction: { commandId: freeze.action.commandId, result: "started" },
    }))
    const progressed = await waitProgress(server.api, source.id, enrollment, sequence, healthy,
      (status) => status.retry.lastResult === "recovered"
        && status.receiver.continuityGeneration !== healthy.receiver.continuityGeneration)
    assert(numeric(progressed.receiver.lastVideoPtsNs) > numeric(healthy.receiver.lastVideoPtsNs))
    await bridge.retirePreviousPeer()
    const strictMedia = await captureStrictSourceRecording(server.api, source.id, supervisorBinary)

    processes.signalNative(sender, "SIGKILL"); await processes.wait(sender); sender = null
    const drop = await waitAction(server.api, source.id, enrollment, sequence)
    assert.equal(drop.action.kind, "restartPublisher")
    sequence.value += 1
    const stopped = await server.api.pollRecovery(source.id, enrollment.controllerToken,
      request(enrollment.controllerId, sequence.value, { intentEpoch: "3", localDesired: "stopped", publisherState: "stopped" }))
    assert.equal(stopped.action, null)
    const quiescent = await waitForQuiescentMedia(server.api, source.id)

    assert.equal((await rawPoll(server.origin, source.id, null, enrollment.controllerId)).status, 401)
    assert.equal((await rawPoll(server.origin, "source-other", enrollment.controllerToken, enrollment.controllerId)).status, 401)
    snapshot = await server.api.snapshot()
    assert.equal(snapshot.project.outputs.length, before.project.outputs.length)
    assert.equal(snapshot.program.programRevision, before.program.programRevision)
    const report = { schemaVersion: 1, status: "pass",
      claim: "real-server-synthetic-gstreamer-recreation-not-obs",
      sourceId: source.id, receiverBefore: healthy.receiver, receiverAfter: progressed.receiver,
      freezeRestartAttempt: freeze.action.attempt,
      localActionExecution: "recreated-synthetic-srt-sender-through-authorized-bridge-handover",
      counterProgressAfterResume: true, recoveredClaim: true, bridgeHandover: handover,
      dropQueuedThenManualStop: true,
      policyVersion: policy.after.policyVersion, quiescentReceiver: quiescent.receiver,
      strictMedia,
      outputCountUnchanged: true, programRevisionUnchanged: true }
    await writeFile(join(directory, "recovery-http-report.json"), `${JSON.stringify(report, null, 2)}\n`, { mode: 0o600 })
  } finally {
    if (sender !== null) await processes.stop(sender)
    await processes.stopAll()
    if (bridge !== null) await bridge.close()
  }
})

function request(controllerId, pollSeq, overrides = {}) {
  return { schemaVersion: 1, controllerId, processEpoch: "1", pollSeq: `${pollSeq}`, intentEpoch: "1",
    localDesired: "stopped", publisherState: "stopped", lastAction: null, ...overrides }
}

async function startSender(processes, root, directory, endpoint, label) {
  const uri = `srt://127.0.0.1:${endpoint.port}?mode=caller&passphrase=${endpoint.passphrase}&pbkeylen=16&streamid=${endpoint.streamId}`
  return processes.spawn(`http-recovery-sender-${label}`, "gst-launch-1.0", ["-q", "-e", "mpegtsmux", "name=mux", "alignment=7",
    "!", "srtsink", `uri=${uri}`, "wait-for-connection=true", "auto-reconnect=false",
    "videotestsrc", "is-live=true", "pattern=solid-color", "foreground-color=4294901760", "!",
    "video/x-raw,width=640,height=360,framerate=30/1", "!", "timeoverlay", "font-desc=Sans 48",
    "shaded-background=true", "valignment=bottom", "!", "videoconvert", "!",
    "video/x-raw,format=I420", "!", "x264enc", "tune=zerolatency", "key-int-max=30", "bitrate=1000",
    "byte-stream=true", "option-string=repeat-headers=1", "!", "h264parse", "config-interval=-1", "!", "mux.",
    "audiotestsrc", "is-live=true", "wave=sine", "freq=440", "!", "audio/x-raw,rate=48000,channels=2", "!",
    "voaacenc", "bitrate=128000", "!", "aacparse", "!", "mux."],
  { cwd: root, env: process.env, logPath: join(directory, `${label}-sender.log`) })
}

async function waitConnected(api, sourceId) {
  await poll(async () => (await api.snapshot()).project.sources.find((source) => source.id === sourceId)
    ?.tracks.every((track) => track.connection === "connected") === true, 20_000, "recovery source connection")
}

async function waitPublisherReady(api, sourceId) {
  await poll(async () => (await api.snapshot()).project.sources.find((source) => source.id === sourceId)
    ?.tracks.every((track) => track.connection === "connecting") === true, 20_000, "recovery source listener")
}

async function stopSenderGracefully(processes, sender) {
  processes.signalNative(sender, "SIGCONT")
  processes.signalNative(sender, "SIGINT")
  await processes.wait(sender)
}

async function waitRecovery(api, sourceId, predicate) {
  let observed
  await poll(async () => { observed = await api.recoveryStatus(sourceId); return predicate(observed) }, 20_000, "recovery status")
  return observed
}

async function waitAction(api, sourceId, enrollment, sequence) {
  let observed
  await poll(async () => {
    sequence.value += 1
    observed = await api.pollRecovery(sourceId, enrollment.controllerToken,
      request(enrollment.controllerId, sequence.value, { intentEpoch: "2", localDesired: "running", publisherState: "failed" }))
    return observed.action !== null
  }, 20_000, "recovery restart action")
  return observed
}

async function waitProgress(api, sourceId, enrollment, sequence, before, predicate = () => true) {
  let observed; const tracker = createProgressTracker(before.receiver)
  await poll(async () => {
    sequence.value += 1
    observed = await api.pollRecovery(sourceId, enrollment.controllerToken,
      request(enrollment.controllerId, sequence.value, { intentEpoch: "2", localDesired: "running", publisherState: "sending" }))
    return tracker.observe(observed.receiver) && predicate(observed)
  }, 20_000, "same-generation receiver counter and PTS progress")
  return observed
}

async function rawPoll(origin, sourceId, token, controllerId) {
  return fetch(`${origin}/api/v1/source-recovery/${sourceId}/poll`, { method: "POST",
    headers: { "content-type": "application/json", ...(token === null ? {} : { authorization: `Bearer ${token}` }) },
    body: JSON.stringify(request(controllerId, 1)) })
}

function numeric(value) { return Number.parseInt(value ?? "0", 10) }
function env(name) { const value = process.env[name]; if (value === undefined) throw new Error(`${name} is required`); return value }
