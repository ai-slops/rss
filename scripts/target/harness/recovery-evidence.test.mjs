import assert from "node:assert/strict"
import test from "node:test"
import { createProgressTracker, receiverSignature, updateRecoveryPolicy } from "./recovery-evidence.mjs"

test("policy mutation uses the fresh observed revision and requires monotonic advance", async () => {
  const calls = []
  const api = {
    recoveryStatus: async () => ({ policyVersion: "7", policy: { enabled: true, maintenance: false } }),
    updateRecovery: async (_sourceId, body) => { calls.push(body); return { policyVersion: "8", policy: body } },
  }
  const result = await updateRecoveryPolicy(api, "source-a", { maintenance: true })
  assert.equal(result.after.policyVersion, "8")
  assert.deepEqual(calls, [{ expectedPolicyVersion: "7", enabled: true, maintenance: true }])
})

test("usable recovery requires two same-generation counter and PTS advances", () => {
  const tracker = createProgressTracker(receiver("1", 100, 200, 300))
  assert.equal(tracker.observe(receiver("2", 1, 1, 1)), false)
  assert.equal(tracker.observe(receiver("2", 2, 2, 2)), false)
  assert.equal(tracker.observe(receiver("2", 3, 3, 3)), true)
  assert.equal(tracker.observe({ ...receiver("2", 4, 4, 4), state: "warming" }), false)
})

test("quiescence signature includes generation, both counters and PTS", () => {
  assert.notEqual(receiverSignature(receiver("1", 1, 2, 3)), receiverSignature(receiver("2", 1, 2, 3)))
  assert.notEqual(receiverSignature(receiver("1", 1, 2, 3)), receiverSignature(receiver("1", 2, 2, 3)))
  assert.notEqual(receiverSignature(receiver("1", 1, 2, 3)), receiverSignature(receiver("1", 1, 3, 3)))
  assert.notEqual(receiverSignature(receiver("1", 1, 2, 3)), receiverSignature(receiver("1", 1, 2, 4)))
})

function receiver(generation, video, audio, pts) {
  return { state: "healthy", continuityGeneration: generation, videoFrames: `${video}`,
    audioBuffers: `${audio}`, lastVideoPtsNs: `${pts}` }
}
