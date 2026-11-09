import assert from "node:assert/strict"
import { dirname, join } from "node:path"
import { readJson, writePrivateJson } from "./private-files.mjs"
import { poll } from "./processes.mjs"
import { captureStrictSourceRecording, waitForQuiescentMedia, waitForRecoveredMedia } from "./recovery-evidence.mjs"
import { waitPluginReport, waitRecoveryStatus } from "./recovery-native-evidence.mjs"

export async function runRequiredPublisherFreeze(fixture, issue) {
  return runPublisherFreeze(fixture, issue)
}

export async function runPublisherFreeze(fixture, issue) {
  const { server, source, bridge, faults, supervisorBinary } = fixture
  const before = await readJson(faults.reportFile)
  const receiverBefore = (await server.api.recoveryStatus(source.id)).receiver
  const bridgeBefore = bridge.stats()
  assert.equal(bridgeBefore.dataPaused, false)
  const proofFile = join(dirname(faults.reportFile), "publisher-freeze-progress.json")
  await issue("pauseActionDispatch", "paused")
  const acknowledgement = await issue("freezePublisher", null, 30000)
  assert.equal(acknowledgement.executedCommandResult, "executed",
    `Publisher freeze hook failed: ${acknowledgement.executedCommandResult}`)
  await writePrivateJson(proofFile, { phase: "writerQuiescentAcknowledged", acknowledgement,
    controllerPollBefore: before.pollSeq, bridge: bridge.stats() })
  const samples = []
  const observedApi = { recoveryStatus: async id => {
    const status = await server.api.recoveryStatus(id)
    assert.equal(status.controller.online, true)
    samples.push({ time: new Date().toISOString(), receiver: status.receiver })
    return status
  } }
  await waitRecoveryStatus(observedApi, source.id, value => value.receiver.state === "stalled", 12000)
  const quiescent = await waitForQuiescentMedia(observedApi, source.id)
  const frozenBridge = bridge.stats()
  let pending
  await poll(async () => {
    await observedApi.recoveryStatus(source.id)
    pending = await readJson(faults.reportFile)
    return pending.recoveryActionPending && pending.pendingRecoveryCommandId.length > 0
      && BigInt(pending.pollSeq) > BigInt(before.pollSeq)
  }, 10000, "pending recovery with controller continuously online")
  assert.equal(bridge.stats().dataPaused, false)
  assert.equal(bridge.stats().forwardedData, frozenBridge.forwardedData,
    "publisher DATA advanced after receiver drain while recovery was held")
  assert.equal(bridge.stats().droppedData, bridgeBefore.droppedData,
    "bridge must not simulate the publisher fault")
  await writePrivateJson(proofFile, { phase: "controllerOnlineReceiverQuiescent", acknowledgement,
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
  const strictMedia = await captureStrictSourceRecording(server.api, source.id, supervisorBinary, "Publisher freeze recovered OBS")
  const afterRecording = await waitForRecoveredMedia(server.api, source.id, recovered.receiver, value =>
    value.controller.online && value.receiver.continuityGeneration === recovered.receiver.continuityGeneration)
  assert(afterRecording.controller.online)
  assert.equal((await readJson(faults.reportFile)).restartExecutionCount, restarted.restartExecutionCount)
  return { kind: "publisherTransportFreeze", mechanism: "ownedPublisherWriterFreeze",
    independentFaultMechanism: true, result: "controllerOnlineQuiescentThenRecoveredUsableAv",
    freezeCommandId: acknowledgement.executedCommandId, controllerPollBefore: before.pollSeq,
    controllerPollDuringFreeze: pending.pollSeq, quiescentReceiver: quiescent.receiver, samples,
    generationBefore: receiverBefore.continuityGeneration, generationAfter: recovered.receiver.continuityGeneration,
    restartExecutionsBefore: before.restartExecutionCount, restartExecutionsAfter: restarted.restartExecutionCount,
    bridgeHandover: handover, strictMedia }
}
