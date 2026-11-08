import assert from "node:assert/strict"
import { readdir, readFile, readlink } from "node:fs/promises"
import { basename, dirname, join } from "node:path"
import { readJson, writePrivateJson } from "./private-files.mjs"
import { poll } from "./processes.mjs"
import { captureStrictSourceRecording, waitForQuiescentMedia, waitForRecoveredMedia } from "./recovery-evidence.mjs"
import { waitPluginReport, waitRecoveryStatus } from "./recovery-native-evidence.mjs"

export async function runRequiredPublisherFreeze(fixture, issue) {
  try {
    return await runPublisherFreeze(fixture, issue)
  } catch (error) {
    if (error.cause?.publisherTransportUnavailable !== true) throw error
    await issue("resumeActionDispatch", "resumed")
    return { kind: "publisherTransportFreeze", result: "unavailable", evidence: error.cause }
  }
}

export async function runPublisherFreeze(fixture, issue) {
  const { server, source, obs, bridge, faults, supervisorBinary } = fixture
  const before = await readJson(faults.reportFile)
  const receiverBefore = (await server.api.recoveryStatus(source.id)).receiver
  const processTree = await observeProcesses(obs.nativePid)
  assert(processTree.muxes.length <= 1, "ambiguous publisher transports cannot be injected")
  const transportBefore = processTree.muxes[0] ?? null
  if (transportBefore !== null) assert.notEqual(transportBefore.state, "T")
  const proofFile = join(dirname(faults.reportFile), "publisher-freeze-progress.json")
  await writePrivateJson(proofFile, { phase: "beforeHook", processTree })
  assert.equal(bridge.stats().dataPaused, false)
  await issue("pauseActionDispatch", "paused")
  const acknowledgement = await issue("freezePublisher", null, 30000)
  if (acknowledgement.executedCommandResult !== "executed") {
    const observed = { phase: "hookRejected", processTree, acknowledgement }
    await writePrivateJson(proofFile, observed)
    if (acknowledgement.executedCommandResult === "publisherTransportNotIdentified") {
      throw new Error("OBS30 publisher transport was not identified by the production freeze hook", {
        cause: { publisherTransportUnavailable: true, processTree,
          hookResult: acknowledgement.executedCommandResult },
      })
    }
    throw new Error(`Publisher freeze hook failed: ${acknowledgement.executedCommandResult}`)
  }
  assert.notEqual(transportBefore, null)
  let frozen
  await poll(async () => {
    frozen = await ownedMux(obs.nativePid)
    return frozen.state === "T"
  }, 3000, "exact OBS mux child stopped")
  assert.equal(frozen.pid, transportBefore.pid)
  assert.equal(frozen.startTicks, transportBefore.startTicks)
  await writePrivateJson(proofFile, { phase: "exactChildFrozen", transportBefore, frozen, controllerPollBefore: before.pollSeq })
  const samples = []
  const observedApi = { recoveryStatus: async id => {
    const status = await server.api.recoveryStatus(id)
    assert.equal(status.controller.online, true)
    samples.push({ time: new Date().toISOString(), receiver: status.receiver })
    return status
  } }
  await waitRecoveryStatus(observedApi, source.id, value => value.receiver.state === "stalled", 12000)
  const quiescent = await waitForQuiescentMedia(observedApi, source.id)
  let pending
  await poll(async () => {
    await observedApi.recoveryStatus(source.id)
    pending = await readJson(faults.reportFile)
    return pending.recoveryActionPending && pending.pendingRecoveryCommandId.length > 0
      && BigInt(pending.pollSeq) > BigInt(before.pollSeq)
  }, 10000, "pending recovery with controller continuously online")
  assert.equal(bridge.stats().dataPaused, false)
  assert.equal((await ownedMux(obs.nativePid)).state, "T")
  await writePrivateJson(proofFile, { phase: "controllerOnlineReceiverQuiescent", transportBefore, frozen,
    controllerPollBefore: before.pollSeq, controllerPollDuringFreeze: pending.pollSeq, samples })
  const handoverPromise = bridge.authorizeHandover()
  await observedApi.recoveryStatus(source.id)
  await issue("resumeActionDispatch", "resumed")
  const handover = await handoverPromise
  assert.notEqual(handover.oldPeerPort, handover.newPeerPort)
  assert.notEqual(handover.oldServerPort, handover.newServerPort)
  const restarted = await waitPluginReport(faults.reportFile, value =>
    Number(value.restartExecutionCount) === Number(before.restartExecutionCount) + 1
      && value.lastRecoveryCommandId === pending.pendingRecoveryCommandId, 15000)
  const recovered = await waitForRecoveredMedia(server.api, source.id, receiverBefore, value =>
    value.retry.lastResult === "recovered" && value.receiver.continuityGeneration !== receiverBefore.continuityGeneration)
  await bridge.retirePreviousPeer()
  const transportAfter = await ownedMux(obs.nativePid)
  assert.notEqual(`${transportAfter.pid}:${transportAfter.startTicks}`, `${transportBefore.pid}:${transportBefore.startTicks}`)
  const strictMedia = await captureStrictSourceRecording(server.api, source.id, supervisorBinary, "Publisher freeze recovered OBS")
  const afterRecording = await waitForRecoveredMedia(server.api, source.id, recovered.receiver, value =>
    value.controller.online && value.receiver.continuityGeneration === recovered.receiver.continuityGeneration)
  assert(afterRecording.controller.online)
  const stableTransport = await ownedMux(obs.nativePid)
  assert.equal(stableTransport.pid, transportAfter.pid)
  assert.equal(stableTransport.startTicks, transportAfter.startTicks)
  assert.notEqual(stableTransport.state, "T")
  assert.equal((await readJson(faults.reportFile)).restartExecutionCount, restarted.restartExecutionCount)
  return { kind: "publisherTransportFreeze", mechanism: "exactOwnedMuxChildPidfdSigstop",
    independentFaultMechanism: true, result: "controllerOnlineQuiescentThenRecoveredUsableAv",
    transportBefore, frozen, transportAfter, controllerPollBefore: before.pollSeq,
    controllerPollDuringFreeze: pending.pollSeq, quiescentReceiver: quiescent.receiver, samples,
    generationBefore: receiverBefore.continuityGeneration, generationAfter: recovered.receiver.continuityGeneration,
    restartExecutionsBefore: before.restartExecutionCount, restartExecutionsAfter: restarted.restartExecutionCount,
    bridgeHandover: handover, strictMedia }
}

async function ownedMux(parentPid) {
  const { muxes } = await observeProcesses(parentPid)
  assert.equal(muxes.length, 1, "one exact owned OBS mux child is required")
  return muxes[0]
}

async function observeProcesses(parentPid) {
  const children = []
  for (const pid of await readdir("/proc")) {
    if (!/^\d+$/u.test(pid)) continue
    try {
      const text = await readFile(`/proc/${pid}/stat`, "utf8")
      const fields = text.slice(text.lastIndexOf(") ") + 2).trim().split(/\s+/u)
      if (Number(fields[1]) !== parentPid) continue
      const executable = basename(await readlink(`/proc/${pid}/exe`))
      children.push({ pid: Number(pid), startTicks: fields[19], state: fields[0], executable })
    } catch (error) {
      if (error.code !== "ENOENT" && error.code !== "ESRCH") throw error
    }
  }
  return { parentPid, parentExecutable: basename(await readlink(`/proc/${parentPid}/exe`)),
    children, muxes: children.filter(child => child.executable === "obs-ffmpeg-mux") }
}
