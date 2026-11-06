import assert from "node:assert/strict"
import { startServerAt } from "./native-runtime.mjs"
import { readJson } from "./private-files.mjs"
import { assertProgramPreserved } from "./program-content-fence.mjs"
import { captureStrictSourceRecording, waitForRecoveredMedia } from "./recovery-evidence.mjs"
import { waitPluginReport, waitRecoveryStatus } from "./recovery-native-evidence.mjs"

export async function restartRecoveryDaemon(fixture) {
  const { server, source, faults, bridge, processes, options, privateDirectory, password, baseline } = fixture
  const beforeStatus = await server.api.recoveryStatus(source.id)
  const beforePlugin = await readJson(faults.reportFile)
  const beforeSnapshot = await server.api.snapshot()
  assert.equal(beforeSnapshot.engineRunId, baseline.snapshot.engineRunId)
  assertProgramPreserved(baseline.snapshot, beforeSnapshot)
  const handoverPromise = bridge.authorizeHandover(60000)
  await processes.stop(server.record)
  const restarted = await startServerAt(processes, options, privateDirectory, password,
    "recovery-restarted", server.stateDirectory, server.port)
  const startup = await waitRecoveryStatus(restarted.api, source.id, value => value.controller.enrolled, 20000)
  assert.equal(startup.retry.attempts, beforeStatus.retry.attempts)
  const freshPlugin = await waitPluginReport(faults.reportFile, value =>
    BigInt(value.pollSeq) > BigInt(beforePlugin.pollSeq), 20000)
  assert.equal(freshPlugin.restartExecutionCount, beforePlugin.restartExecutionCount)
  const snapshot = await restarted.api.snapshot()
  assert.notEqual(snapshot.engineRunId, beforeSnapshot.engineRunId)
  assertProgramPreserved(baseline.snapshot, snapshot)
  const recovered = await waitForRecoveredMedia(restarted.api, source.id, startup.receiver,
    value => value.controller.online)
  bridge.cancelHandover()
  const handover = await handoverPromise
  await bridge.retirePreviousPeer()
  const strictMedia = await captureStrictSourceRecording(restarted.api, source.id,
    options.supervisorBinary, "Daemon restart recovered OBS")
  return { server: restarted, snapshot, evidence: { kind: "observeServerRestart",
    result: "controllerPersistedAndRecoveredUsableAv", attempts: startup.retry.attempts,
    staleActionExecutions: 0, engineRunBefore: beforeSnapshot.engineRunId,
    engineRunAfter: snapshot.engineRunId, receiver: recovered.receiver, bridgeHandover: handover, strictMedia } }
}
