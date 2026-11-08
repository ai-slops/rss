import assert from "node:assert/strict"
import { randomUUID } from "node:crypto"
import { readJson } from "./private-files.mjs"
import { waitPluginReport, waitRecoveryStatus } from "./recovery-native-evidence.mjs"
import { waitForQuiescentMedia } from "./recovery-evidence.mjs"

export async function stopQueuedRecovery(fixture, issue) {
  const { server, source, faults, bridge } = fixture
  await issue("pauseActionDispatch", "paused")
  await server.api.retryRecovery(source.id, randomUUID())
  await waitRecoveryStatus(server.api, source.id, status => status.retry.lastResult === "pending", 10000)
  const pending = await waitPluginReport(faults.reportFile, report => report.recoveryActionPending
    && report.pendingRecoveryCommandId.length > 0, 10000)
  const beforeStop = Number(pending.restartExecutionCount)
  await issue("manualStopAfterQueuedRestart", "executed")
  await issue("resumeActionDispatch", "alreadyCancelled")
  await waitPluginReport(faults.reportFile, report => report.localDesired === "stopped"
    && !report.recoveryActionPending && report.pendingRecoveryCommandId === ""
    && Number(report.restartExecutionCount) === beforeStop, 10000)
  bridge.resumeData()
  const stopped = await waitForQuiescentMedia(server.api, source.id)
  await new Promise(resolve => setTimeout(resolve, 500))
  assert.equal(Number((await readJson(faults.reportFile)).restartExecutionCount), beforeStop)
  return { kind: "manualStopAfterQueuedRestart", result: "pendingCancelledStoppedNoDecodedProgress",
    pendingRecoveryCommandId: pending.pendingRecoveryCommandId, receiver: stopped.receiver }
}
