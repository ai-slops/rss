import { resolve } from "node:path"
import { writePrivateJson } from "./private-files.mjs"

export const recoveryFaultKinds = Object.freeze([
  "dropPublisher",
  "freezePublisher",
  "manualStopAfterQueuedRestart",
  "pollMissingAuthorization",
  "pollWrongSourceAuthorization",
  "exhaustRetryBudget",
  "reconnectController",
  "redeliverDuplicateAction",
  "observeServerRestart",
  "setMaintenance",
  "setSourceDisabled",
  "letControllerLeaseExpire",
  "pauseActionDispatch",
  "resumeActionDispatch",
])

export async function writeRecoveryFaultFixture(directory, input) {
  const path = resolve(directory, "obs-recovery-fixture.json")
  const reportFile = resolve(directory, "obs-recovery-report.json")
  const commandFile = resolve(directory, "obs-recovery-command.json")
  await writePrivateJson(path, {
    schemaVersion: 1,
    localFixtureOnly: true,
    sourceId: input.sourceId,
    publisherBundle: resolve(input.publisherBundle),
    reportFile,
    commandFile,
    deadlineSeconds: input.deadlineSeconds,
    operations: recoveryFaultKinds.map((kind) => kind === "freezePublisher"
      ? { kind, durationMs: 4_000 }
      : { kind }),
  })
  return { fixtureFile: path, reportFile, commandFile }
}

export async function writeRecoveryFaultCommand(path, commandId, kind, durationMs = null) {
  if (!/^[1-9]\d*$/u.test(commandId) || !recoveryFaultKinds.includes(kind)) {
    throw new Error("Recovery command ID or operation is outside the finite contract")
  }
  const command = { schemaVersion: 1, commandId, kind }
  if (durationMs !== null) {
    if (!Number.isInteger(durationMs) || durationMs < 1 || durationMs > 30_000) {
      throw new Error("Recovery command durationMs must be 1 through 30000")
    }
    command.durationMs = durationMs
  }
  await writePrivateJson(path, command)
  return command
}

export function validateFaultFixture(value, sourceId) {
  if (value?.schemaVersion !== 1 || value.localFixtureOnly !== true || value.sourceId !== sourceId) {
    throw new Error("Recovery fault fixture is not source-scoped local schema version 1")
  }
  const kinds = value.operations?.map((entry) => entry.kind)
  if (JSON.stringify(kinds) !== JSON.stringify(recoveryFaultKinds)) {
    throw new Error("Recovery fault fixture operations differ from the finite allowlist")
  }
  if (value.operations.some((entry) => Object.hasOwn(entry, "command") || Object.hasOwn(entry, "url"))) {
    throw new Error("Recovery fault fixture must not contain generic commands or URLs")
  }
  if (typeof value.commandFile !== "string" || value.commandFile.length === 0) {
    throw new Error("Recovery fault fixture omitted its private commandFile")
  }
  return value
}
