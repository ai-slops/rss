import { join, resolve } from "node:path"
import { readJson, writePrivateJson } from "./private-files.mjs"

export async function writeFixtureFiles(directory, senders, declarations, deadlineSeconds) {
  const reportFile = resolve(join(directory, "obs-fixture-report.json"))
  const controlFile = resolve(join(directory, "obs-fixture-control.json"))
  const fixtureFile = resolve(join(directory, "obs-fixture.json"))
  await writePrivateJson(controlFile, {
    schemaVersion: 1,
    commands: declarations.map((declaration) => declaration.kind === undefined
      ? { kind: "configureNativeFixtureSource", ...declaration }
      : declaration),
  })
  await writePrivateJson(fixtureFile, {
    schemaVersion: 1,
    localFixtureOnly: true,
    senders: senders.map((sender) => resolve(sender)),
    reportFile,
    controlFile,
    deadlineSeconds,
  })
  return { fixtureFile, reportFile, controlFile }
}

export async function validateFixtureReport(path, bundles) {
  const report = await readJson(path)
  if (report.schemaVersion !== 1 || report.moduleLoaded !== true
    || report.module !== "obs-live-production" || report.state !== "ready") {
    throw new Error("OBS fixture report does not prove module load")
  }
  if (!Array.isArray(report.senders) || report.senders.length < bundles.length) {
    throw new Error("OBS fixture report omitted configured senders")
  }
  for (const bundle of bundles) {
    const sender = report.senders.find((candidate) => candidate.sourceId === bundle.target.sourceId)
    if (sender?.state !== "sending") throw new Error(`OBS sender ${bundle.target.sourceId} is not sending`)
    if (sender.audioMixIndex !== bundle.obs.audioMixIndex) throw new Error("OBS audio mix routing differs from bundle")
  }
  const badAuth = report.negativeCases?.find((candidate) => candidate.kind === "badAuth")
  const duplicate = report.negativeCases?.find((candidate) => candidate.kind === "duplicateSender")
  if (badAuth?.result !== "rejected" || duplicate?.result !== "rejected") {
    throw new Error("OBS fixture report omitted bad-auth or duplicate-sender rejection")
  }
  return report
}
