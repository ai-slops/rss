import assert from "node:assert/strict"
import { randomUUID } from "node:crypto"
import { join } from "node:path"
import { readJson, writePrivateJson } from "./private-files.mjs"
import { OwnedProcesses, waitForFile } from "./processes.mjs"
import { startServerAt } from "./native-runtime.mjs"
import { writeRecoveryFaultCommand } from "./recovery-faults.mjs"
import { captureStrictSourceRecording, updateRecoveryPolicy, waitForQuiescentMedia, waitForRecoveredMedia } from "./recovery-evidence.mjs"
import { startRecoveryFixture } from "./recovery-native-setup.mjs"
import { diagnosticOmissions, recoveryDisposition } from "./recovery-diagnostic-profile.mjs"
import { runNativeSafety } from "./recovery-native-safety.mjs"
import { assertProgramPreserved } from "./program-content-fence.mjs"
import { numeric as number, rawControllerPoll as rawPoll, usableReceiver as usable,
  waitPluginReport as waitReport, waitRecoveryStatus as waitStatus } from "./recovery-native-evidence.mjs"

export async function runRecoveryNative(options, evidenceDirectory, privateDirectory) {
  if (process.env.LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_MODE !== undefined) return runNativeSafety(options, evidenceDirectory, privateDirectory)
  const processes = new OwnedProcesses(options.supervisorBinary,
    { timeoutMs: options.deadlineSeconds * 1_000, graceMs: 15_000 })
  let server; let obs; let password; let bridge = null; let commandId = 0; const faultMatrix = []
  const omissions = diagnosticOmissions(process.env.LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_OMIT)
  let phase = "initialMedia"; let observedSourceId = null
  try {
    const fixture = await startRecoveryFixture(options, privateDirectory, processes)
    ;({ server, obs, bridge, password } = fixture)
    const { source, baseline, policy, enrollment, faults } = fixture
    let snapshot
    observedSourceId = source.id
    await waitForFile(faults.reportFile, 30_000, obs)
    let plugin = await waitReport(faults.reportFile,
      report => report.recoveryReady === true && report.localDesired === "running", 30_000)
    const healthy = await waitStatus(server.api, source.id, status => usable(status.receiver), 30_000)

    phase = "bridgeDataDrop"
    await issue(faults, ++commandId, "pauseActionDispatch", "paused")
    const beforeDropCount = number(plugin.restartExecutionCount)
    const beforeDropAttempts = (await server.api.recoveryStatus(source.id)).retry.attempts
    const beforeDroppedDatagrams = bridge.stats().droppedData
    bridge.pauseData()
    await waitStatus(server.api, source.id,
      (status) => status.controller.online === true && status.receiver.state === "stalled", 12_000)
    assert(bridge.stats().droppedData > beforeDroppedDatagrams)
    plugin = await waitReport(faults.reportFile,
      (report) => report.recoveryActionPending === true && report.pendingRecoveryCommandId.length > 0, 10_000)
    const pendingDropCommand = plugin.pendingRecoveryCommandId
    assert.equal((await server.api.recoveryStatus(source.id)).retry.lastResult, "pending")
    const dropHandoverPromise = bridge.authorizeHandover()
    await issue(faults, ++commandId, "resumeActionDispatch", "resumed")
    const dropHandover = await dropHandoverPromise
    plugin = await waitReport(faults.reportFile,
      (report) => number(report.restartExecutionCount) > beforeDropCount
        && report.lastRecoveryCommandId === pendingDropCommand, 15_000)
    bridge.resumeData()
    const recovered = await waitForRecoveredMedia(server.api, source.id, healthy.receiver,
      (status) => status.retry.lastResult === "recovered"
        && status.receiver.continuityGeneration !== healthy.receiver.continuityGeneration)
    await bridge.retirePreviousPeer()
    const strictMedia = await captureStrictSourceRecording(server.api, source.id, options.supervisorBinary,
      "Recovery OBS strict clean")
    faultMatrix.push({ kind: "bridgeDataDrop", mechanism: "loopbackSrtDataPause", result: "recoveredUsableAv",
      videoFrames: recovered.receiver.videoFrames, audioBuffers: recovered.receiver.audioBuffers,
      continuityGenerationBefore: healthy.receiver.continuityGeneration,
      continuityGenerationAfter: recovered.receiver.continuityGeneration,
      attemptsBefore: beforeDropAttempts, droppedDatagrams: bridge.stats().droppedData - beforeDroppedDatagrams,
      pendingRecoveryCommandId: pendingDropCommand, bridgeHandover: dropHandover })

    const beforeDuplicate = number(plugin.restartExecutionCount)
    const completedCommandId = plugin.lastRecoveryCommandId
    const completedResult = plugin.lastRecoveryResult
    await issue(faults, ++commandId, "redeliverDuplicateAction", "duplicateAcknowledged")
    plugin = await readJson(faults.reportFile)
    assert.equal(number(plugin.restartExecutionCount), beforeDuplicate)
    assert.equal(plugin.duplicateCommandId, completedCommandId)
    assert.equal(plugin.duplicateResult, completedResult)
    if (plugin.duplicateCommandId.length === 0) throw new Error("Duplicate action was not actually presented")
    faultMatrix.push({ kind: "redeliverDuplicateAction", result: "sameOutcomeNoAdditionalExecution",
      commandId: plugin.duplicateCommandId, outcome: plugin.duplicateResult })

    if (!omissions.includes("receiverDataFreeze")) {
      phase = "receiverDataFreeze"
      await issue(faults, ++commandId, "pauseActionDispatch", "paused")
      const beforeFreezeCount = number(plugin.restartExecutionCount)
      const beforeFreezeReceiver = (await server.api.recoveryStatus(source.id)).receiver
      const beforeFreezeDrops = bridge.stats().droppedData
      bridge.pauseData()
      await waitStatus(server.api, source.id,
        (status) => status.controller.online === true && status.receiver.state === "stalled", 12_000)
      assert(bridge.stats().droppedData > beforeFreezeDrops)
      plugin = await waitReport(faults.reportFile,
        (report) => report.recoveryActionPending === true && report.pendingRecoveryCommandId.length > 0, 10_000)
      const freezeHandoverPromise = bridge.authorizeHandover()
      await issue(faults, ++commandId, "resumeActionDispatch", "resumed")
      const freezeHandover = await freezeHandoverPromise
      plugin = await waitReport(faults.reportFile,
        (report) => number(report.restartExecutionCount) > beforeFreezeCount, 15_000)
      bridge.resumeData()
      const thawed = await waitForRecoveredMedia(server.api, source.id, beforeFreezeReceiver,
        (status) => status.receiver.continuityGeneration !== beforeFreezeReceiver.continuityGeneration)
      await bridge.retirePreviousPeer()
      plugin = await readJson(faults.reportFile)
      faultMatrix.push({ kind: "receiverDataFreeze", mechanism: "loopbackSrtDataPause", independentFaultMechanism: false, result: "dataPausedControllerOnlineRecovered",
        droppedDatagrams: bridge.stats().droppedData - beforeFreezeDrops,
        restartExecutionsBefore: beforeFreezeCount, restartExecutionsAfter: number(plugin.restartExecutionCount),
        generationBefore: beforeFreezeReceiver.continuityGeneration,
        generationAfter: thawed.receiver.continuityGeneration, bridgeHandover: freezeHandover,
        receiver: thawed.receiver })
    }

    if (!omissions.includes("obsProcessSigstop")) {
      phase = "obsProcessSigstop"
      const beforeProcessStopPoll = number(plugin.pollSeq)
      const beforeProcessStopCount = number(plugin.restartExecutionCount)
      const beforeProcessStopReceiver = (await server.api.recoveryStatus(source.id)).receiver
      processes.signalNative(obs, "SIGSTOP")
      await waitStatus(server.api, source.id,
        (status) => status.controller.online === false && status.receiver.state === "controller_offline", 12_000)
      processes.signalNative(obs, "SIGCONT")
      plugin = await waitReport(faults.reportFile,
        (report) => number(report.pollSeq) > beforeProcessStopPoll, 15_000)
      await waitStatus(server.api, source.id, (status) => status.controller.online === true, 15_000)
      const resumedProcess = await waitForRecoveredMedia(server.api, source.id, beforeProcessStopReceiver)
      plugin = await readJson(faults.reportFile)
      faultMatrix.push({ kind: "obsProcessSigstop", result: "offlineThenResumedUsableAv",
        processRestartClaim: false, restartExecutionsBefore: beforeProcessStopCount,
        restartExecutionsAfter: number(plugin.restartExecutionCount), receiver: resumedProcess.receiver })
    }

    phase = "authorizationAndServerRestart"
    await issue(faults, ++commandId, "pollMissingAuthorization", "awaitingHttpEvidence")
    assert.equal((await rawPoll(server.origin, source.id, null, enrollment.controllerId)).status, 401)
    faultMatrix.push({ kind: "pollMissingAuthorization", result: "http401" })
    await issue(faults, ++commandId, "pollWrongSourceAuthorization", "awaitingHttpEvidence")
    assert.equal((await rawPoll(server.origin, "wrong-source", enrollment.controllerToken, enrollment.controllerId)).status, 401)
    faultMatrix.push({ kind: "pollWrongSourceAuthorization", result: "http401" })

    await issue(faults, ++commandId, "observeServerRestart", "awaitingHttpEvidence")
    const beforeRestartStatus = await server.api.recoveryStatus(source.id)
    const beforeRestartPlugin = await readJson(faults.reportFile)
    const stateDirectory = server.stateDirectory; const serverPort = server.port
    await processes.stop(server.record)
    server = await startServerAt(processes, options, privateDirectory, password,
      "recovery-restarted", stateDirectory, serverPort)
    const restartedStatus = await waitStatus(server.api, source.id, (status) => status.controller.enrolled === true, 20_000)
    assert.equal(restartedStatus.retry.attempts, beforeRestartStatus.retry.attempts)
    plugin = await waitReport(faults.reportFile,
      (report) => number(report.pollSeq) > number(beforeRestartPlugin.pollSeq), 20_000)
    assert.equal(number(plugin.restartExecutionCount), number(beforeRestartPlugin.restartExecutionCount))
    faultMatrix.push({ kind: "observeServerRestart", result: "controllerPersisted",
      attempts: restartedStatus.retry.attempts, staleActionExecutions: 0 })

    phase = "controllerReconnect"
    await issue(faults, ++commandId, "reconnectController", "paused", 1_000)
    const pausedPoll = number((await readJson(faults.reportFile)).pollSeq)
    await waitReport(faults.reportFile, (report) => number(report.pollSeq) >= pausedPoll + 3, 10_000)
    faultMatrix.push({ kind: "reconnectController", result: "pollSequenceAdvanced" })
    await issue(faults, ++commandId, "letControllerLeaseExpire", "paused", 6_000)
    await waitStatus(server.api, source.id, (status) => status.controller.online === false, 10_000)
    await waitStatus(server.api, source.id, (status) => status.controller.online === true, 12_000)
    faultMatrix.push({ kind: "letControllerLeaseExpire", result: "offlineThenOnline" })

    phase = "maintenance"
    await issue(faults, ++commandId, "setMaintenance", "awaitingHttpEvidence")
    const maintenance = await updateRecoveryPolicy(server.api, source.id, { maintenance: true })
    assert.equal(maintenance.after.policy.maintenance, true)
    const maintenanceCount = number((await readJson(faults.reportFile)).restartExecutionCount)
    const maintenanceAttempts = (await server.api.recoveryStatus(source.id)).retry.attempts
    const maintenanceDrops = bridge.stats().droppedData
    bridge.pauseData()
    await waitStatus(server.api, source.id,
      (status) => status.controller.online === true && status.receiver.state === "stalled", 10_000)
    assert(bridge.stats().droppedData > maintenanceDrops)
    assert.equal(number((await readJson(faults.reportFile)).restartExecutionCount), maintenanceCount)
    assert.equal((await server.api.recoveryStatus(source.id)).retry.attempts, maintenanceAttempts)
    const maintainedVersion = maintenance.after.policyVersion
    const restored = await updateRecoveryPolicy(server.api, source.id, { maintenance: false })
    bridge.resumeData()
    await waitForRecoveredMedia(server.api, source.id, (await server.api.recoveryStatus(source.id)).receiver)
    faultMatrix.push({ kind: "setMaintenance", result: "zeroRestartExecutions",
      enabledVersion: maintainedVersion, restoredVersion: restored.after.policyVersion })

    phase = "sourceDisabled"
    await issue(faults, ++commandId, "setSourceDisabled", "awaitingHttpEvidence")
    const disabledCount = number((await readJson(faults.reportFile)).restartExecutionCount)
    const disabledAttempts = (await server.api.recoveryStatus(source.id)).retry.attempts
    bridge.pauseData()
    snapshot = await server.api.snapshot(); const current = snapshot.project.sources.find((candidate) => candidate.id === source.id)
    await server.api.command("source.configure", source.id, { source: { ...current, enabled: false } },
      { snapshot, expectedRevision: snapshot.project.revision })
    assert.equal((await server.api.recoveryStatus(source.id)).receiver.state, "source_disabled")
    await new Promise((resolve) => setTimeout(resolve, 3_500))
    assert.equal(number((await readJson(faults.reportFile)).restartExecutionCount), disabledCount)
    assert.equal((await server.api.recoveryStatus(source.id)).retry.attempts, disabledAttempts)
    snapshot = await server.api.snapshot(); const disabled = snapshot.project.sources.find((candidate) => candidate.id === source.id)
    await server.api.command("source.configure", source.id, { source: { ...disabled, enabled: true } },
      { snapshot, expectedRevision: snapshot.project.revision })
    bridge.resumeData()
    await waitForRecoveredMedia(server.api, source.id, (await server.api.recoveryStatus(source.id)).receiver)
    faultMatrix.push({ kind: "setSourceDisabled", result: "sourceDisabledZeroActions" })

    phase = "retryExhaustion"
    await issue(faults, ++commandId, "exhaustRetryBudget", "armedThreeFailures")
    await issue(faults, ++commandId, "dropPublisher", "executed")
    const exhausted = await waitStatus(server.api, source.id,
      (status) => status.retry.lastResult === "exhausted" && status.retry.attempts === 3, 30_000)
    faultMatrix.push({ kind: "exhaustRetryBudget", result: "exhausted", attempts: exhausted.retry.attempts })

    phase = "manualStop"
    await issue(faults, ++commandId, "pauseActionDispatch", "paused")
    await server.api.retryRecovery(source.id, randomUUID())
    await waitStatus(server.api, source.id, (status) => status.retry.lastResult === "pending", 10_000)
    plugin = await waitReport(faults.reportFile,
      (report) => report.recoveryActionPending === true && report.pendingRecoveryCommandId.length > 0, 10_000)
    const pendingStopCommand = plugin.pendingRecoveryCommandId
    const beforeStop = number((await readJson(faults.reportFile)).restartExecutionCount)
    await issue(faults, ++commandId, "manualStopAfterQueuedRestart", "executed")
    await issue(faults, ++commandId, "resumeActionDispatch", "alreadyCancelled")
    plugin = await waitReport(faults.reportFile,
      (report) => report.localDesired === "stopped" && report.recoveryActionPending === false
        && report.pendingRecoveryCommandId === "" && number(report.restartExecutionCount) === beforeStop, 10_000)
    bridge.resumeData()
    const stoppedAfter = await waitForQuiescentMedia(server.api, source.id)
    await new Promise((resolve) => setTimeout(resolve, 500))
    assert.equal(number((await readJson(faults.reportFile)).restartExecutionCount), beforeStop)
    faultMatrix.push({ kind: "manualStopAfterQueuedRestart", result: "pendingCancelledStoppedNoDecodedProgress",
      pendingRecoveryCommandId: pendingStopCommand })

    snapshot = await server.api.snapshot()
    assert.equal(snapshot.project.outputs.length, baseline.outputs)
    assertProgramPreserved(baseline.snapshot, snapshot)
    const report = { schemaVersion: 1, ...recoveryDisposition(omissions, ["publisherTransportFreeze"]), nativeExecution: true,
      sourceId: source.id, initialReceiver: healthy.receiver, recoveredReceiver: recovered.receiver,
      policyVersion: restored.after.policyVersion, strictMedia,
      exhaustedAttempts: exhausted.retry.attempts, restartExecutionCount: number(plugin.restartExecutionCount),
      lastCommandId: plugin.executedCommandId, faultMatrix,
      udpBridge: bridge.stats(),
      outputCountUnchanged: true, programContentUnchanged: true,
      programRevisionUnchanged: snapshot.program.programRevision === baseline.programRevision }
    await writePrivateJson(join(evidenceDirectory, "recovery-report.json"), report)
    return report
  } catch (error) {
    await writePrivateJson(join(privateDirectory, "failure-progress.json"), { phase, diagnosticOmissions: omissions,
      faultMatrix, detail: error.message, observations: error.cause ?? null,
      lastStatus: server === undefined || observedSourceId === null ? null : await server.api.recoveryStatus(observedSourceId).catch(() => null),
      bridge: bridge?.stats() ?? null }).catch(() => undefined)
    throw new Error(error instanceof Error ? error.message : "Native recovery execution failed", {
      cause: { nativeExecution: obs !== undefined },
    })
  } finally {
    await processes.stopAll()
    if (bridge !== null) await bridge.close()
  }

  async function issue(faults, id, kind, result, durationMs = null) {
    await writeRecoveryFaultCommand(faults.commandFile, `${id}`, kind, durationMs)
    return waitReport(faults.reportFile,
      (report) => report.executedCommandId === `${id}` && report.executedCommandKind === kind
        && report.executedCommandResult === result, 15_000)
  }
}
