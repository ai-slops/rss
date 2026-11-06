import assert from "node:assert/strict"
import { randomUUID } from "node:crypto"
import { join } from "node:path"
import { setTimeout as delay } from "node:timers/promises"
import { readFile } from "node:fs/promises"
import { OwnedProcesses, waitForFile, poll } from "./processes.mjs"
import { startObs, startServerAt } from "./native-runtime.mjs"
import { startRecoveryFixture } from "./recovery-native-setup.mjs"
import { readJson, writePrivateJson } from "./private-files.mjs"
import { writeRecoveryFaultCommand } from "./recovery-faults.mjs"
import { waitPluginReport, waitRecoveryStatus, usableReceiver, rawControllerPoll } from "./recovery-native-evidence.mjs"
import { captureStrictSourceRecording, waitForQuiescentMedia, updateRecoveryPolicy } from "./recovery-evidence.mjs"

export async function runNativeSafety(options, evidenceDirectory, privateDirectory) {
  const mode = process.env.LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_MODE
  if (!["fences", "budget", "controls"].includes(mode)) throw new Error("Unsupported native diagnostic mode")
  const processes = new OwnedProcesses(options.supervisorBinary, { timeoutMs: options.deadlineSeconds * 1000, graceMs: 15000 })
  let fixture; let memoryBefore; let commandId = 0; const cases = []
  try {
    fixture = await startRecoveryFixture(options, privateDirectory, processes)
    const { server, source, faults, enrollment } = fixture
    await waitForFile(faults.reportFile, 30000, fixture.obs)
    await waitPluginReport(faults.reportFile, value => value.recoveryReady && value.localDesired === "running", 30000)
    await waitRecoveryStatus(server.api, source.id, value => usableReceiver(value.receiver), 30000)
    memoryBefore = { obs: await memory(fixture.obs.nativePid), server: await memory(server.record.nativePid) }
    try {
      cases.push({ kind: "initialStrictMedia", status: "pass",
        evidence: await captureStrictSourceRecording(server.api, source.id, options.supervisorBinary, "Safety strict media") })
    } catch (error) { cases.push({ kind: "initialStrictMedia", status: "failed", detail: error.message }) }
    switch (mode) {
      case "fences": await fences(); break
      case "budget": await budget(); break
      case "controls": await controls(); break
      default: throw new Error("Unsupported native diagnostic mode")
    }
    const snapshot = await fixture.server.api.snapshot()
    assert.equal(snapshot.project.outputs.length, fixture.baseline.outputs)
    assert.equal(snapshot.program.programRevision, fixture.baseline.programRevision)
    const report = { schemaVersion: 1, status: "partial", nativeExecution: true, obsClaim: false,
      diagnosticMode: mode, originalFullGateRequired: true, cases,
      memoryBefore, memoryAfter: { obs: await memory(fixture.obs.nativePid), server: await memory(fixture.server.record.nativePid) },
      outputCountUnchanged: true, programRevisionUnchanged: true }
    await writePrivateJson(join(evidenceDirectory, "recovery-report.json"), report)
    return report
  } catch (error) {
    await writePrivateJson(join(privateDirectory, "safety-progress.json"), { diagnosticMode: mode, cases,
      detail: error.message }).catch(() => undefined)
    throw new Error(error.message, { cause: { nativeExecution: fixture !== undefined } })
  } finally {
    await processes.stopAll()
    if (fixture !== undefined) await fixture.bridge.close()
  }

  async function issue(kind, result, durationMs = null) {
    await writeRecoveryFaultCommand(fixture.faults.commandFile, `${++commandId}`, kind, durationMs)
    return waitPluginReport(fixture.faults.reportFile, value => value.executedCommandId === `${commandId}`
      && value.executedCommandKind === kind && value.executedCommandResult === result, 15000)
  }

  async function stopQueued() {
    const before = await readJson(fixture.faults.reportFile)
    assert.equal(before.recoveryActionPending, true)
    assert.notEqual(before.pendingRecoveryCommandId, "")
    await issue("manualStopAfterQueuedRestart", "executed")
    await issue("resumeActionDispatch", "alreadyCancelled")
    const after = await waitPluginReport(fixture.faults.reportFile, value => value.localDesired === "stopped"
      && !value.recoveryActionPending && value.pendingRecoveryCommandId === "", 10000)
    assert.equal(after.restartExecutionCount, before.restartExecutionCount)
    fixture.bridge.resumeData()
    await waitForQuiescentMedia(fixture.server.api, fixture.source.id)
    cases.push({ kind: "queuedManualStop", status: "pass", cancelledCommandId: before.pendingRecoveryCommandId,
      executionsBefore: before.restartExecutionCount, executionsAfter: after.restartExecutionCount })
  }

  async function fences() {
    const { server, source, enrollment, faults } = fixture
    for (const [kind, id, token] of [["missingAuthorization", source.id, null],
      ["wrongAuthorization", source.id, "invalid-fixture-token"], ["wrongSource", "not-this-source", enrollment.controllerToken]]) {
      const response = await rawControllerPoll(server.origin, id, token, enrollment.controllerId)
      assert.equal(response.status, 401)
      cases.push({ kind, status: "pass", httpStatus: response.status })
    }
    await issue("pauseActionDispatch", "paused")
    fixture.bridge.pauseData()
    await waitRecoveryStatus(server.api, source.id, value => value.receiver.state === "stalled", 12000)
    await waitPluginReport(faults.reportFile, value => value.recoveryActionPending && value.pendingRecoveryCommandId, 10000)
    await stopQueued()
    const requestId = randomUUID()
    for (let index = 0; index < 2; index++) await assert.rejects(server.api.retryRecovery(source.id, requestId), /HTTP 409/u)
    cases.push({ kind: "stoppedRemoteRetryFailsClosed", status: "pass", attempts: 2 })
    const originalPid = fixture.obs.nativePid
    processes.signalNative(fixture.obs, "SIGKILL")
    await processes.wait(fixture.obs)
    const offline = await waitRecoveryStatus(server.api, source.id, value => !value.controller.online
      && value.receiver.state === "controller_offline", 12000)
    assert.equal(offline.capabilities.processRestart, false)
    cases.push({ kind: "deadObsReportedOffline", status: "pass", processRestartClaim: false })
    const replacement = await startObs(processes, options, privateDirectory, "", fixture.display)
    fixture.obs = replacement
    assert.notEqual(replacement.nativePid, originalPid)
    await delay(2000)
    const stopped = await waitForQuiescentMedia(server.api, source.id)
    assert.equal(stopped.controller.localDesired, "stopped")
    cases.push({ kind: "freshObsNoBackendMediaResume", status: "pass", pluginStoppedUiClaim: false, oldPid: originalPid, newPid: replacement.nativePid,
      receiver: stopped.receiver })
    const epoch = `${BigInt(stopped.controller.processEpoch ?? "1") + 1n}`
    const intent = `${BigInt((await readJson(faults.reportFile)).intentEpoch) + 1n}`
    const body = { schemaVersion: 1, controllerId: enrollment.controllerId, processEpoch: epoch,
      pollSeq: "1", intentEpoch: intent, localDesired: "running", publisherState: "sending", lastAction: null }
    const registered = await server.api.pollRecovery(source.id, enrollment.controllerToken, body)
    assert.equal(registered.processEpoch, epoch)
    assert.equal((await server.api.recoveryStatus(source.id)).controller.localDesired, "stopped")
    assert.equal(registered.action, null)
    const duplicate = await server.api.pollRecovery(source.id, enrollment.controllerToken, body)
    assert.deepEqual(duplicate, registered)
    const raw = async value => fetch(`${server.origin}/api/v1/source-recovery/${source.id}/poll`, { method: "POST",
      headers: { "content-type": "application/json", authorization: `Bearer ${enrollment.controllerToken}` },
      body: JSON.stringify(value), redirect: "error" })
    assert.equal((await raw({ ...body, publisherState: "failed" })).status, 409)
    assert.equal((await raw({ ...body, processEpoch: `${BigInt(epoch) - 1n}`, pollSeq: "999" })).status, 409)
    await server.api.pollRecovery(source.id, enrollment.controllerToken, { ...body, pollSeq: "2", intentEpoch: `${BigInt(intent) + 1n}`, localDesired: "stopped", publisherState: "stopped" })
    assert.equal((await raw({ ...body, intentEpoch: `${BigInt(intent) + 1n}`, localDesired: "stopped", publisherState: "stopped" })).status, 409)
    assert.equal((await raw({ ...body, pollSeq: "3" })).status, 409)
    cases.push({ kind: "processPollIntentFencesAndDuplicate", status: "pass", recreatedProcessEpoch: epoch,
      staleProcess: 409, stalePoll: 409, staleIntent: 409, changedDuplicate: 409, exactDuplicate: "sameResponse", freshProcessCannotArm: true })
    const rotated = await server.api.enrollRecovery(source.id, true)
    assert.notEqual(rotated.controllerId, enrollment.controllerId)
    assert.equal((await rawControllerPoll(server.origin, source.id, enrollment.controllerToken, enrollment.controllerId)).status, 401)
    cases.push({ kind: "rotatedControllerRejectsOldToken", status: "pass", httpStatus: 401 })
  }

  async function budget() {
    const { server, source, faults } = fixture
    await issue("exhaustRetryBudget", "armedThreeFailures")
    await issue("dropPublisher", "executed")
    const timeline = []; const firstSeen = new Map()
    await poll(async () => {
      const value = await server.api.recoveryStatus(source.id)
      const sample = { timeMs: Date.now(), ...value.retry }
      timeline.push(sample)
      assert(value.retry.attempts <= 3)
      if (value.retry.attempts > 0 && !firstSeen.has(value.retry.attempts)) firstSeen.set(value.retry.attempts, sample)
      return value.retry.lastResult === "exhausted" && value.retry.attempts === 3
    }, 30000, "three bounded native recovery attempts")
    assert.deepEqual([...firstSeen.keys()], [1, 2, 3])
    const observed = [...firstSeen.values()]
    for (let index = 0; index < observed.length; index++) {
      const sample = observed[index]
      const remainingNs = BigInt(sample.nextEligibleAtNs) - BigInt(sample.timeMs) * 1000000n
      assertNativeBackoff(index + 1, remainingNs)
      const following = observed[index + 1] ?? timeline.at(-1)
      assert(BigInt(following.timeMs) * 1000000n >= BigInt(sample.nextEligibleAtNs) - 100000000n)
    }
    const plugin = await readJson(faults.reportFile)
    assert.equal(plugin.restartExecutionCount, 3)
    cases.push({ kind: "nativeRetryBudgetAndBackoff", status: "pass", maximumAttempts: 3,
      executionCount: plugin.restartExecutionCount, sampledEligibility: observed, exhaustedAt: timeline.at(-1) })
    await issue("pauseActionDispatch", "paused")
    await issue("reconnectController", "paused", 1000)
    const requestId = randomUUID()
    const replies = await Promise.all([server.api.retryRecovery(source.id, requestId), server.api.retryRecovery(source.id, requestId)])
    assert.deepEqual(replies[0].retry, replies[1].retry)
    assert.equal(replies[0].retry.attempts, 0)
    await waitPluginReport(faults.reportFile, value => value.recoveryActionPending && value.pendingRecoveryCommandId, 10000)
    assert.equal((await readJson(faults.reportFile)).restartExecutionCount, 3)
    cases.push({ kind: "concurrentDuplicateManualRetry", status: "pass", resetBudgetOnce: true, executionsHeld: 3 })
    await stopQueued()
  }

  async function controls() {
    let { server } = fixture
    const { source, faults, bridge } = fixture
    const initialPlugin = await readJson(faults.reportFile)
    await issue("reconnectController", "paused", 1000)
    await waitPluginReport(faults.reportFile, value => BigInt(value.pollSeq) >= BigInt(initialPlugin.pollSeq) + 3n, 10000)
    cases.push({ kind: "controllerReconnect", status: "pass" })
    await issue("letControllerLeaseExpire", "paused", 6000)
    await waitRecoveryStatus(server.api, source.id, value => !value.controller.online, 10000)
    await waitRecoveryStatus(server.api, source.id, value => value.controller.online, 12000)
    cases.push({ kind: "controllerLeaseOfflineThenOnline", status: "pass", processRestartClaim: false })
    await updateRecoveryPolicy(server.api, source.id, { maintenance: true })
    const before = await readJson(faults.reportFile)
    const attempts = (await server.api.recoveryStatus(source.id)).retry.attempts
    bridge.pauseData()
    await waitRecoveryStatus(server.api, source.id, value => value.controller.online && value.receiver.state === "stalled", 12000)
    await delay(1000)
    assert.equal((await readJson(faults.reportFile)).restartExecutionCount, before.restartExecutionCount)
    assert.equal((await server.api.recoveryStatus(source.id)).retry.attempts, attempts)
    cases.push({ kind: "maintenanceSuppressesRecovery", status: "pass", attempts, executions: before.restartExecutionCount })
    bridge.resumeData()
    await waitRecoveryStatus(server.api, source.id, value => value.receiver.state === "healthy", 12000)
    let snapshot = await server.api.snapshot()
    const current = snapshot.project.sources.find(value => value.id === source.id)
    await server.api.command("source.configure", source.id, { source: { ...current, enabled: false } }, { snapshot, expectedRevision: snapshot.project.revision })
    assert.equal((await server.api.recoveryStatus(source.id)).receiver.state, "source_disabled")
    await updateRecoveryPolicy(server.api, source.id, { maintenance: false })
    await delay(3500)
    assert.equal((await readJson(faults.reportFile)).restartExecutionCount, before.restartExecutionCount)
    assert.equal((await server.api.recoveryStatus(source.id)).retry.attempts, attempts)
    cases.push({ kind: "sourceDisabledSuppressesRecovery", status: "pass", attempts, maintenanceAlsoEnabled: false })
    await updateRecoveryPolicy(server.api, source.id, { maintenance: true })
    snapshot = await server.api.snapshot()
    const disabled = snapshot.project.sources.find(value => value.id === source.id)
    await server.api.command("source.configure", source.id, { source: { ...disabled, enabled: true } }, { snapshot, expectedRevision: snapshot.project.revision })
    const oldRun = (await server.api.snapshot()).engineRunId
    const oldPlugin = await readJson(faults.reportFile)
    await processes.stop(server.record)
    server = await startServerAt(processes, options, privateDirectory, fixture.password, "safety-restarted", server.stateDirectory, server.port)
    fixture.server = server
    assert.notEqual((await server.api.snapshot()).engineRunId, oldRun)
    await waitPluginReport(faults.reportFile, value => BigInt(value.pollSeq) > BigInt(oldPlugin.pollSeq), 20000)
    await waitRecoveryStatus(server.api, source.id, value => value.controller.online, 20000)
    assert.equal((await server.api.recoveryStatus(source.id)).retry.attempts, attempts)
    assert.equal((await readJson(faults.reportFile)).restartExecutionCount, oldPlugin.restartExecutionCount)
    cases.push({ kind: "serverRestartFencesStaleActions", status: "pass", maintenanceEnabled: true, retryAttemptsPreserved: attempts })
    await issue("manualStopAfterQueuedRestart", "executed")
    await waitForQuiescentMedia(server.api, source.id)
  }

  async function memory(pid) {
    const status = await readFile(`/proc/${pid}/status`, "utf8")
    return { pid, rssKiB: Number(status.match(/^VmRSS:\s+(\d+)/mu)?.[1]), peakRssKiB: Number(status.match(/^VmHWM:\s+(\d+)/mu)?.[1]) }
  }
}

export function assertNativeBackoff(attempt, remainingNs) {
  const expected = BigInt(2 ** (attempt - 1)) * 1000000000n
  assert(remainingNs >= expected - 500000000n && remainingNs <= expected + 100000000n)
}
