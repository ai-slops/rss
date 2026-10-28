import { join, resolve } from "node:path"
import { randomBytes } from "node:crypto"
import { parseArgs, usage } from "./harness/args.mjs"
import { readJson, redactEndpoint, validateFreshEvidenceDirectory, writePrivateJson } from "./harness/private-files.mjs"
import { OwnedProcesses, freeUdpPort, waitForFile } from "./harness/processes.mjs"
import { createInvalidAuthBundle, writePublisherBundle } from "./harness/publisher-bundle.mjs"
import { validateFixtureReport, writeFixtureFiles } from "./harness/fixture-contract.mjs"
import { startObs, startServer, startVirtualDisplay } from "./harness/native-runtime.mjs"
import {
  configureProduction, createRecordingOutputs, createSrtSources, exportEditorSession,
  startEditorSession, startOutputs, stopOutputs, waitForConnectedSources,
} from "./harness/workflow-api.mjs"
import { analyzeSnapshotRecordings } from "./harness/recording-evidence.mjs"
import { proveAbruptRecovery } from "./harness/workflow-crash.mjs"
import { buildPublicReport, dryRunReport } from "./harness/public-report.mjs"

const root = resolve(new URL("../..", import.meta.url).pathname)
let options
try {
  options = { ...parseArgs(process.argv.slice(2), root), root }
} catch (error) {
  process.stderr.write(`${error.message}\n${usage()}\n`)
  process.exit(2)
}
const prepared = await validateFreshEvidenceDirectory(
  options.evidenceDirectory,
  options.dryRun ? [] : ["preflight.json"],
)
const { evidenceDirectory, privateDirectory } = prepared

if (options.dryRun) {
  await writePrivateJson(join(evidenceDirectory, "rehearsal-report.json"), dryRunReport(options))
  process.stdout.write(`target_rehearsal=partial_native_not_run report=${join(evidenceDirectory, "rehearsal-report.json")}\n`)
  process.exit(0)
}
if (process.env.LIVE_PRODUCTION_OWNER_PID === undefined
  || process.env.LIVE_PRODUCTION_OWNER_START_TICKS === undefined) {
  throw new Error("Actual rehearsal must be launched through rehearsal.sh and the native supervisor")
}

const preflight = await requirePreflight(options.preflightReport)
const processes = new OwnedProcesses(options.supervisorBinary, {
  timeoutMs: options.deadlineSeconds * 1000,
  graceMs: 15_000,
})
const password = randomBytes(32).toString("hex")
await writePrivateJson(join(privateDirectory, "operator-auth.json"), { password })
let server
try {
  server = await startServer(processes, options, privateDirectory, password, "initial")
  const ports = [await freeUdpPort(), await freeUdpPort()]
  const sources = await createSrtSources(server.api, ports)
  const snapshot = await server.api.snapshot()
  const bundleRecords = []
  const declarations = []
  for (const source of sources) {
    const endpoint = await server.api.publishingEndpoint(source.source.id)
    const record = await writePublisherBundle(privateDirectory, source.key, {
      snapshot,
      endpoint,
      host: options.publisherHost,
      sourceName: source.name,
      audioMixIndex: source.audioMixIndex,
      encoding: {
        width: 1280, height: 720, fpsNumerator: 30, fpsDenominator: 1, videoBitrateKbps: 2500,
      },
    })
    bundleRecords.push({ ...record, endpoint: redactEndpoint(endpoint), declaration: source })
    declarations.push({
      senderBundle: record.path,
      sourceId: source.source.id,
      sourceName: source.name,
      color: source.color,
      frameCounter: true,
      toneHz: source.toneHz,
      audioMixIndex: source.audioMixIndex,
    })
  }
  const invalidBundle = createInvalidAuthBundle(bundleRecords[0].bundle)
  const invalidPath = join(privateDirectory, "bad-auth.publisher.json")
  await writePrivateJson(invalidPath, invalidBundle)
  declarations.push({ kind: "startBadAuthSender", senderBundle: invalidPath, expected: "rejected" })
  declarations.push({ kind: "startDuplicateSender", senderBundle: bundleRecords[0].path, expected: "rejected" })
  const fixture = await writeFixtureFiles(privateDirectory, bundleRecords.map((record) => record.path), declarations, options.deadlineSeconds)
  const display = await startVirtualDisplay(processes, options, privateDirectory)
  await startObs(processes, options, privateDirectory, fixture.fixtureFile, display.display)
  await waitForFile(fixture.reportFile, options.deadlineSeconds * 1000)
  const fixtureReport = await validateFixtureReport(fixture.reportFile, bundleRecords.map((record) => record.bundle))
  validateDeclaredContent(fixtureReport, declarations)
  await waitForConnectedSources(server.api, sources, 30_000)
  const production = await configureProduction(server.api, sources)
  await startEditorSession(server.api, "target-rehearsal")
  const outputs = await createRecordingOutputs(server.api, sources)
  await startOutputs(server.api, outputs)
  await new Promise((resolve) => setTimeout(resolve, 8_000))
  const recordedSnapshot = await stopOutputs(server.api, outputs)
  const recordingEvidence = await analyzeSnapshotRecordings(
    recordedSnapshot,
    sources.map(({ name, color, toneHz }) => ({ name, color, toneHz })),
    options.supervisorBinary,
  )
  const crash = await proveAbruptRecovery({
    api: server.api, processes, server, options, privateDirectory, password, stateDirectory: server.stateDirectory,
  })
  server = crash.restarted
  const exported = await exportEditorSession(server.api)
  assertExport(exported, crash)
  const report = buildPublicReport({ options, preflight, sources, bundleRecords, fixtureReport, production, recordingEvidence, crash, exported, evidenceDirectory })
  await writePrivateJson(join(evidenceDirectory, "rehearsal-report.json"), report)
  process.stdout.write(`target_rehearsal=passed report=${join(evidenceDirectory, "rehearsal-report.json")}\n`)
} finally {
  await processes.stopAll()
}

async function requirePreflight(path) {
  if (path === null) throw new Error("Actual rehearsal requires --preflight-report from preflight.sh")
  const report = await readJson(path)
  if (report.status !== "pass") throw new Error("Native Ubuntu preflight did not pass")
  return report
}

function validateDeclaredContent(report, declarations) {
  for (const declaration of declarations.filter((entry) => entry.kind === undefined)) {
    const sender = report.senders.find((entry) => entry.sourceId === declaration.sourceId)
    if (sender?.visual?.color !== declaration.color || sender.visual.frameCounter !== true) {
      throw new Error(`OBS native visual declaration missing for ${declaration.sourceId}`)
    }
    if (sender.audio?.toneHz !== declaration.toneHz) throw new Error(`OBS tone declaration missing for ${declaration.sourceId}`)
  }
}

function assertExport(exported, crash) {
  if (!Array.isArray(exported.mediaInventory) || exported.mediaInventory.length === 0) throw new Error("Editor export has no media inventory")
  if (!exported.gaps.some((gap) => gap.kind === "recording" && gap.detail === "abrupt_shutdown")) {
    throw new Error("Editor export omitted abrupt recording gap")
  }
  if (!exported.mediaInventory.some((entry) => entry.recording.outputId === crash.outputId)) {
    throw new Error("Editor export omitted recovered crash recording")
  }
}
