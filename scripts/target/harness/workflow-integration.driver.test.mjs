import assert from "node:assert/strict"
import test from "node:test"
import { mkdtemp, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { randomBytes } from "node:crypto"
import { OwnedProcesses, freeUdpPort } from "./processes.mjs"
import { startServer } from "./native-runtime.mjs"
import {
  configureProduction, createRecordingOutputs, createSrtSources, exportEditorSession,
  startEditorSession, startOutputs, stopOutputs, waitForConnectedSources,
} from "./workflow-api.mjs"
import { analyzeSnapshotRecordings } from "./recording-evidence.mjs"
import { proveAbruptRecovery } from "./workflow-crash.mjs"

const enabled = process.env.LIVE_PRODUCTION_REHEARSAL_DRIVER === "1"

test("drives real HTTP workflow with authenticated synthetic SRT media", { skip: !enabled, timeout: 180_000 }, async () => {
  const root = process.env.LIVE_PRODUCTION_REHEARSAL_ROOT
  const serverBinary = process.env.LIVE_PRODUCTION_TEST_SERVER
  const supervisorBinary = process.env.LIVE_PRODUCTION_TEST_SUPERVISOR
  const webDist = process.env.LIVE_PRODUCTION_TEST_WEB_DIST
  if ([root, serverBinary, supervisorBinary, webDist].some((value) => value === undefined)) {
    throw new Error("Integration driver environment is incomplete")
  }
  const privateDirectory = await mkdtemp(join(root, "var/target-rehearsal-driver."))
  const options = { root, serverBinary, supervisorBinary, webDist, deadlineSeconds: 120 }
  const processes = new OwnedProcesses(supervisorBinary, { timeoutMs: 120_000, graceMs: 10_000 })
  const password = randomBytes(32).toString("hex")
  let server
  try {
    server = await startServer(processes, options, privateDirectory, password, "driver")
    const sources = await createSrtSources(server.api, [await freeUdpPort(), await freeUdpPort()])
    const endpoints = await Promise.all(sources.map(({ source }) => server.api.publishingEndpoint(source.id)))
    for (let index = 0; index < sources.length; index += 1) {
      await startSyntheticSender(processes, options, privateDirectory, sources[index], endpoints[index])
    }
    await waitForConnectedSources(server.api, sources, 30_000)
    const production = await configureProduction(server.api, sources)
    assert.notEqual(production.snapshot.program.programRevision, production.programRevisionBeforeDraft)
    await startEditorSession(server.api, "synthetic-srt-driver")
    const outputs = await createRecordingOutputs(server.api, sources)
    await startOutputs(server.api, outputs)
    await new Promise((resolve) => setTimeout(resolve, 8_000))
    const recorded = await stopOutputs(server.api, outputs)
    const media = await analyzeSnapshotRecordings(
      recorded,
      sources.map(({ name, color, toneHz }) => ({ name, color, toneHz })),
      supervisorBinary,
    )
    assert.equal(media.length, 3)
    const crash = await proveAbruptRecovery({
      api: server.api, processes, server, options, privateDirectory, password, stateDirectory: server.stateDirectory,
    })
    server = crash.restarted
    const exported = await exportEditorSession(server.api)
    assert(exported.mediaInventory.length >= crash.rawFragments)
    assert(exported.gaps.some((gap) => gap.kind === "recording" && gap.detail === "abrupt_shutdown"))
    const result = {
      status: "pass",
      senderKind: "supervised-gstreamer-synthetic-not-obs",
      sourceIds: sources.map(({ source }) => source.id),
      outputs: media.map(({ output, recordings }) => ({ id: output.id, kind: output.adapter.kind, segments: recordings.length })),
      crash: {
        rawFragments: crash.rawFragments,
        confirmedClosedBeforeKill: crash.confirmedClosedBeforeKill,
        hashesPreserved: crash.hashesPreserved,
      },
      exportInventory: exported.mediaInventory.length,
    }
    await writeFile(join(privateDirectory, "driver-result.json"), `${JSON.stringify(result, null, 2)}\n`, { mode: 0o600 })
    assert.equal(JSON.stringify(result).includes("passphrase"), false)
  } finally {
    await processes.stopAll()
  }
})

async function startSyntheticSender(processes, options, directory, declaration, endpoint) {
  const color = declaration.color === "red" ? 4_294_901_760 : 4_278_190_335
  const uri = `srt://127.0.0.1:${endpoint.port}?mode=caller&passphrase=${endpoint.passphrase}&pbkeylen=16&streamid=${endpoint.streamId}`
  const pipeline = [
    "-q", "-e", "mpegtsmux", "name=mux", "!", "srtsink", `uri=${uri}`, "wait-for-connection=false", "auto-reconnect=true",
    "videotestsrc", "is-live=true", "pattern=solid-color", `foreground-color=${color}`, "!",
    "video/x-raw,width=1280,height=720,framerate=30/1", "!", "timeoverlay", "!",
    "x264enc", "tune=zerolatency", "key-int-max=30", "bitrate=2500", "!", "h264parse", "config-interval=-1", "!", "mux.",
    "audiotestsrc", "is-live=true", "wave=sine", `freq=${declaration.toneHz}`, "!",
    "audio/x-raw,rate=48000,channels=2", "!", "voaacenc", "bitrate=128000", "!", "aacparse", "!", "mux.",
  ]
  return processes.spawn(`sender-${declaration.key}`, "gst-launch-1.0", pipeline, {
    cwd: options.root,
    env: process.env,
    logPath: join(directory, `${declaration.key}-sender.log`),
  })
}
