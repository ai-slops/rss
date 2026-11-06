import assert from "node:assert/strict"
import { readdir, readFile, readlink } from "node:fs/promises"
import { basename, dirname, join } from "node:path"
import { readJson, writePrivateJson } from "./private-files.mjs"
import { poll } from "./processes.mjs"
import { captureStrictSourceRecording, waitForQuiescentMedia, waitForRecoveredMedia } from "./recovery-evidence.mjs"
import { waitPluginReport, waitRecoveryStatus } from "./recovery-native-evidence.mjs"

export async function runPublisherFreeze(fixture, issue) {
  const { server, source, obs, bridge, faults, supervisorBinary } = fixture
  const before = await readJson(faults.reportFile)
  const receiverBefore = (await server.api.recoveryStatus(source.id)).receiver
  const transportBefore = await ownedMux(obs.nativePid)
  assert.equal(bridge.stats().dataPaused, false)
  await issue("pauseActionDispatch", "paused")
  await issue("freezePublisher", "executed", 30000)
  let frozen
  await poll(async () => {
    frozen = await ownedMux(obs.nativePid)
    return frozen.state === "T"
  }, 3000, "exact OBS mux child stopped")
  assert.equal(frozen.pid, transportBefore.pid)
  assert.equal(frozen.startTicks, transportBefore.startTicks)
  const proofFile = join(dirname(faults.reportFile), "publisher-freeze-progress.json")
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
  const pending = await waitPluginReport(faults.reportFile, value => value.recoveryActionPending
    && value.pendingRecoveryCommandId.length > 0 && BigInt(value.pollSeq) > BigInt(before.pollSeq), 10000)
  assert.equal(bridge.stats().dataPaused, false)
  assert.equal((await ownedMux(obs.nativePid)).state, "T")
  await writePrivateJson(proofFile, { phase: "controllerOnlineReceiverQuiescent", transportBefore, frozen,
    controllerPollBefore: before.pollSeq, controllerPollDuringFreeze: pending.pollSeq, samples })
  const handoverPromise = bridge.authorizeHandover()
  await issue("resumeActionDispatch", "resumed")
  const handover = await handoverPromise
  assert.notEqual(handover.oldPeerPort, handover.newPeerPort)
  assert.notEqual(handover.oldServerPort, handover.newServerPort)
  const restarted = await waitPluginReport(faults.reportFile, value =>
    Number(value.restartExecutionCount) > Number(before.restartExecutionCount)
      && value.lastRecoveryCommandId === pending.pendingRecoveryCommandId, 15000)
  const recovered = await waitForRecoveredMedia(server.api, source.id, receiverBefore, value =>
    value.retry.lastResult === "recovered" && value.receiver.continuityGeneration !== receiverBefore.continuityGeneration)
  await bridge.retirePreviousPeer()
  const transportAfter = await ownedMux(obs.nativePid)
  assert.notEqual(`${transportAfter.pid}:${transportAfter.startTicks}`, `${transportBefore.pid}:${transportBefore.startTicks}`)
  const strictMedia = await captureStrictSourceRecording(server.api, source.id, supervisorBinary, "Publisher freeze recovered OBS")
  return { kind: "publisherTransportFreeze", mechanism: "exactOwnedMuxChildPidfdSigstop",
    independentFaultMechanism: true, result: "controllerOnlineQuiescentThenRecoveredUsableAv",
    transportBefore, frozen, transportAfter, controllerPollBefore: before.pollSeq,
    controllerPollDuringFreeze: pending.pollSeq, quiescentReceiver: quiescent.receiver, samples,
    generationBefore: receiverBefore.continuityGeneration, generationAfter: recovered.receiver.continuityGeneration,
    restartExecutionsBefore: before.restartExecutionCount, restartExecutionsAfter: restarted.restartExecutionCount,
    bridgeHandover: handover, strictMedia }
}

async function ownedMux(parentPid) {
  const candidates = []
  for (const pid of await readdir("/proc")) {
    if (!/^\d+$/u.test(pid)) continue
    try {
      const text = await readFile(`/proc/${pid}/stat`, "utf8")
      const fields = text.slice(text.lastIndexOf(") ") + 2).trim().split(/\s+/u)
      if (Number(fields[1]) !== parentPid) continue
      if (basename(await readlink(`/proc/${pid}/exe`)) !== "obs-ffmpeg-mux") continue
      candidates.push({ pid: Number(pid), startTicks: fields[19], state: fields[0] })
    } catch (error) {
      if (error.code !== "ENOENT" && error.code !== "ESRCH") throw error
    }
  }
  assert.equal(candidates.length, 1, "one exact owned OBS mux child is required")
  return candidates[0]
}
