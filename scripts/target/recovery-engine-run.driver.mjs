import assert from "node:assert/strict"
import { randomBytes } from "node:crypto"
import { mkdir, writeFile } from "node:fs/promises"
import { resolve, join } from "node:path"
import { ensureSupervised } from "../runner-supervision.mjs"
import { OwnedProcesses, freeUdpPort, poll } from "./harness/processes.mjs"
import { startServer, startServerAt } from "./harness/native-runtime.mjs"
import { captureStrictSourceRecording, updateRecoveryPolicy } from "./harness/recovery-evidence.mjs"
import { assertProgramPreserved } from "./harness/program-content-fence.mjs"

await ensureSupervised()
const root = resolve(new URL("../..", import.meta.url).pathname)
const [directory] = process.argv.slice(2)
if (!directory) throw new Error("Expected fresh private evidence directory")
await mkdir(directory, { mode: 0o700 })
const options = { root, serverBinary: join(root, "target/release/live-production"),
  supervisorBinary: join(root, "target/release/live-production-supervisor"), webDist: join(root, "web/dist"), deadlineSeconds: 150 }
const processes = new OwnedProcesses(options.supervisorBinary, { timeoutMs: 150000, graceMs: 10000 })
const password = randomBytes(32).toString("hex")
let server; let sender; let endpoint; let enrollment; let sourceId; let seq = 0; let phase = "startup"
const evidence = {}
try {
  server = await startServer(processes, options, directory, password, "run-a")
  const initialProgram = await server.api.snapshot()
  const sameScene = initialProgram.project.scenes.find(value => value.id === initialProgram.program.sceneId)
  await server.api.command("program.take", sameScene.id, { sceneId: sameScene.id, revision: sameScene.revision },
    { snapshot: initialProgram, expectedRevision: initialProgram.program.programRevision })
  const baseline = await server.api.snapshot()
  evidence.sameSceneTakeAdvancedRevision = baseline.program.programRevision !== initialProgram.program.programRevision
  await server.api.command("source.create", baseline.project.id, { name: "Engine-run source",
    adapter: { kind: "srt", bindAddress: "127.0.0.1", port: await freeUdpPort(), bufferMs: 120 } }, { snapshot: baseline })
  sourceId = (await server.api.snapshot()).project.sources.find(value => value.name === "Engine-run source").id
  endpoint = await server.api.publishingEndpoint(sourceId)
  await updateRecoveryPolicy(server.api, sourceId, { enabled: true, maintenance: false })
  enrollment = await server.api.enrollRecovery(sourceId, false)
  await server.api.pollRecovery(sourceId, enrollment.controllerToken, request({ intentEpoch: "1", localDesired: "stopped", publisherState: "stopped" }))
  sender = await startSender("engine-a")
  const originalHealthy = await healthy()
  phase = "oldPending"
  processes.signalNative(sender, "SIGSTOP")
  let oldPoll; let pending
  await poll(async () => {
    oldPoll = request({ publisherState: "failed" })
    pending = await server.api.pollRecovery(sourceId, enrollment.controllerToken, oldPoll)
    return pending.action !== null
  }, 15000, "old-run pending recovery")
  const oldCommand = pending.action.commandId
  evidence.oldAction = { commandId: oldCommand, attempt: pending.action.attempt, serverRunId: pending.action.serverRunId }
  const beforeRestart = await server.api.snapshot()
  await stopSender()
  await processes.stop(server.record)
  phase = "startupBinding"
  server = await startServerAt(processes, options, directory, password, "run-b", server.stateDirectory, server.port)
  const afterRestart = await server.api.snapshot()
  assert.notEqual(afterRestart.engineRunId, beforeRestart.engineRunId)
  assertProgramPreserved(beforeRestart, afterRestart)
  const bound = await server.api.recoveryStatus(sourceId)
  assert.equal(bound.retry.lastResult, "cancelled")
  assert.equal(bound.retry.attempts, pending.action.attempt)
  await assert.rejects(server.api.pollRecovery(sourceId, enrollment.controllerToken, oldPoll), /HTTP 409/u)
  evidence.startup = { oldEngineRunId: beforeRestart.engineRunId, newEngineRunId: afterRestart.engineRunId,
    attempts: bound.retry.attempts, result: bound.retry.lastResult, oldDuplicateRejected: true,
    rawRevisionChanged: beforeRestart.program.programRevision !== afterRestart.program.programRevision,
    programContentPreserved: true }
  phase = "newRunMedia"
  sender = await startSender("engine-b-generation-1")
  const generationOne = await healthy()
  await stopSender()
  await poll(async () => (await server.api.snapshot()).project.sources.find(value => value.id === sourceId)
    .tracks.every(track => track.connection === "connecting"), 10000, "new-run listener released")
  sender = await startSender("engine-b-generation-2")
  const generationTwo = await healthy(value => BigInt(value.receiver.continuityGeneration) > BigInt(generationOne.receiver.continuityGeneration))
  const beforeLateAck = await server.api.recoveryStatus(sourceId)
  phase = "lateOldAck"
  const late = await server.api.pollRecovery(sourceId, enrollment.controllerToken,
    request({ lastAction: { commandId: oldCommand, result: "started" } }))
  assert.notEqual(late.retry.lastResult, "recovered")
  assert.equal(late.retry.attempts, beforeLateAck.retry.attempts)
  if (late.action !== null) {
    assert.notEqual(late.action.commandId, oldCommand)
    assert.notEqual(late.action.serverRunId, pending.action.serverRunId)
  }
  const strictMedia = await captureStrictSourceRecording(server.api, sourceId, options.supervisorBinary, "Engine-run strict media")
  const snapshot = await server.api.snapshot()
  assertProgramPreserved(baseline, snapshot)
  assert.equal(snapshot.project.outputs.length, baseline.project.outputs.length)
  const report = { schemaVersion: 1, status: "pass", claim: "real-backend-gstreamer-engine-run-fence-not-obs", ...evidence,
    originalReceiver: originalHealthy.receiver, newRunFirstReceiver: generationOne.receiver, newRunSecondReceiver: generationTwo.receiver,
    lateOldAckIgnored: true, attemptsBeforeLateAck: beforeLateAck.retry.attempts, attemptsAfterLateAck: late.retry.attempts,
    strictMedia, programContentPreserved: true, outputCountUnchanged: true }
  await writeFile(join(directory, "report.json"), JSON.stringify(report, null, 2), { mode: 0o600 })
} catch (error) {
  await writeFile(join(directory, "failure.json"), JSON.stringify({ phase, evidence, detail: error.message }, null, 2), { mode: 0o600 })
  throw error
} finally {
  await processes.stopAll()
}

function request(overrides = {}) {
  return { schemaVersion: 1, controllerId: enrollment.controllerId, processEpoch: "1", pollSeq: `${++seq}`,
    intentEpoch: "2", localDesired: "running", publisherState: "sending", lastAction: null, ...overrides }
}
async function healthy(predicate = () => true) {
  let value
  await poll(async () => {
    value = await server.api.pollRecovery(sourceId, enrollment.controllerToken, request())
    return value.receiver.state === "healthy" && BigInt(value.receiver.videoFrames) > 10n
      && BigInt(value.receiver.audioBuffers) > 10n && predicate(value)
  }, 15000, "actual decoded receiver health")
  return value
}
async function stopSender() {
  processes.signalNative(sender, "SIGCONT")
  processes.signalNative(sender, "SIGINT")
  await processes.wait(sender)
}
async function startSender(label) {
  return processes.spawn(label, "gst-launch-1.0", ["-q", "-e", "mpegtsmux", "name=mux", "alignment=7", "!", "srtsink",
    `uri=srt://127.0.0.1:${endpoint.port}?mode=caller&passphrase=${endpoint.passphrase}&pbkeylen=16&streamid=${endpoint.streamId}`,
    "wait-for-connection=true", "auto-reconnect=false", "videotestsrc", "is-live=true", "pattern=solid-color", "foreground-color=4294901760",
    "!", "video/x-raw,width=640,height=360,framerate=30/1", "!", "timeoverlay", "font-desc=Sans 48", "shaded-background=true", "valignment=bottom", "!", "videoconvert", "!", "video/x-raw,format=I420",
    "!", "x264enc", "tune=zerolatency", "key-int-max=30", "!", "h264parse", "config-interval=-1", "!", "mux.",
    "audiotestsrc", "is-live=true", "freq=440", "!", "audio/x-raw,rate=48000,channels=2", "!", "voaacenc", "!", "aacparse", "!", "mux."],
  { cwd: root, env: process.env, logPath: join(directory, `${label}.log`) })
}
