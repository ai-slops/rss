import { poll } from "./processes.mjs"
import { analyzeRecordingGroup } from "./media-analysis.mjs"

export async function updateRecoveryPolicy(api, sourceId, changes) {
  const before = await api.recoveryStatus(sourceId)
  const after = await api.updateRecovery(sourceId, {
    expectedPolicyVersion: before.policyVersion,
    enabled: changes.enabled ?? before.policy.enabled,
    maintenance: changes.maintenance ?? before.policy.maintenance,
  })
  if (BigInt(after.policyVersion) <= BigInt(before.policyVersion)) {
    throw new Error("Recovery policy mutation did not advance its revision")
  }
  return { before, after }
}

export function createProgressTracker(initialReceiver) {
  let previous = initialReceiver
  let healthyAdvances = 0
  return {
    observe(receiver) {
      if (receiver.state !== "healthy") { previous = receiver; healthyAdvances = 0; return false }
      if (receiver.continuityGeneration !== previous.continuityGeneration) {
        previous = receiver; healthyAdvances = 0; return false
      }
      if (advanced(receiver, previous)) healthyAdvances += 1
      else healthyAdvances = 0
      previous = receiver
      return healthyAdvances >= 2
    },
  }
}

export async function waitForRecoveredMedia(api, sourceId, initialReceiver, predicate = () => true) {
  const tracker = createProgressTracker(initialReceiver)
  const receiverSamples = []
  let status
  await poll(async () => {
    status = await api.recoveryStatus(sourceId)
    receiverSamples.push({ time: new Date().toISOString(), receiver: status.receiver, retryResult: status.retry.lastResult })
    if (receiverSamples.length > 64) receiverSamples.shift()
    return tracker.observe(status.receiver) && predicate(status)
  }, 30_000, "two healthy same-generation receiver advances").catch(error => {
    throw new Error(error.message, { cause: { receiverSamples } })
  })
  return status
}

export async function waitForQuiescentMedia(api, sourceId, options = {}) {
  const drainMs = options.drainMs ?? 1_500
  const stableMs = options.stableMs ?? 2_000
  await new Promise((resolve) => setTimeout(resolve, drainMs))
  let status = await api.recoveryStatus(sourceId)
  let signature = receiverSignature(status.receiver)
  let stableSince = Date.now()
  await poll(async () => {
    status = await api.recoveryStatus(sourceId)
    const current = receiverSignature(status.receiver)
    if (current !== signature) { signature = current; stableSince = Date.now() }
    return Date.now() - stableSince >= stableMs
  }, options.timeoutMs ?? 10_000, "receiver quiescence after bounded drain")
  return status
}

export function receiverSignature(receiver) {
  return [receiver.continuityGeneration, receiver.videoFrames, receiver.audioBuffers, receiver.lastVideoPtsNs].join(":")
}

export async function captureStrictSourceRecording(api, sourceId, supervisorBinary, name = "Recovery strict clean") {
  let snapshot = await api.snapshot()
  await api.command("output.create", snapshot.project.id,
    { name, adapter: { kind: "sourceRecording", sourceId, segmentSeconds: 1 } }, { snapshot })
  snapshot = await api.snapshot()
  const output = snapshot.project.outputs.find((candidate) => candidate.name === name)
  if (output === undefined) throw new Error("Strict recovery recording output was not created")
  await api.command("output.start", output.id, {}, { snapshot })
  await new Promise((resolve) => setTimeout(resolve, 4_000))
  snapshot = await api.snapshot(); await api.command("output.stop", output.id, {}, { snapshot })
  snapshot = await api.snapshot()
  const analyzed = await analyzeRecordingGroup(
    snapshot.recordings.filter((recording) => recording.outputId === output.id),
    { label: name, profile: { width: 1280, height: 720 },
      content: { kind: "clean", label: name, color: "red", toneHz: 440 } }, supervisorBinary)
  snapshot = await api.snapshot(); await api.command("output.delete", output.id, {}, { snapshot })
  return { segments: analyzed.segments.length, content: analyzed.content,
    video: analyzed.segments[0].video, audio: analyzed.segments[0].audio }
}

function advanced(after, before) {
  return numeric(after.videoFrames) > numeric(before.videoFrames)
    && numeric(after.audioBuffers) > numeric(before.audioBuffers)
    && numeric(after.lastVideoPtsNs) > numeric(before.lastVideoPtsNs)
}

function numeric(value) { return BigInt(value ?? "0") }
