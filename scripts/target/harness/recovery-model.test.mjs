import assert from "node:assert/strict"
import test from "node:test"
import { RecoveryModel } from "./recovery-model.mjs"

const sourceId = "source-a"
const controllerId = "11111111-1111-4111-8111-111111111111"
const token = "source-only-token"

test("authenticates source scope and requires fresh manual running intent per process", () => {
  const model = create()
  assert.equal(model.poll({ sourceId, token: null, request: stopped(), nowMs: 0 }).status, 401)
  assert.equal(model.poll({ sourceId: "source-b", token, request: stopped(), nowMs: 0 }).status, 401)
  assert.equal(model.poll({ sourceId, token, request: stopped(), nowMs: 0 }).body.action, null)
  stalled(model, 3_000)
  assert.equal(model.poll({ sourceId, token, request: poll({ pollSeq: "2" }), nowMs: 3_000 }).body.action, null)
  const armed = model.poll({ sourceId, token, request: poll({ pollSeq: "3", intentEpoch: "2", localDesired: "running" }), nowMs: 3_000 })
  assert.equal(armed.body.action.kind, "restartPublisher")
  assert.equal(armed.body.action.attempt, 1)
  assert.equal(model.poll({ sourceId, token, request: poll({ processEpoch: "0", pollSeq: "4" }), nowMs: 3_001 }).status, 409)
  assert.equal(model.poll({ sourceId, token, request: poll({ pollSeq: "2", intentEpoch: "2" }), nowMs: 3_001 }).status, 409)
})

test("deduplicates actions and fences queued restart with manual Stop and policy changes", () => {
  const model = create(); armAndStall(model)
  const first = model.poll({ sourceId, token, request: poll({ pollSeq: "3", intentEpoch: "2", localDesired: "running" }), nowMs: 3_000 }).body.action
  const duplicate = model.poll({ sourceId, token, request: poll({ pollSeq: "3", intentEpoch: "2", localDesired: "running" }), nowMs: 3_001 }).body.action
  assert.deepEqual(duplicate, first)
  assert.equal(model.status().retry.attempts, 1)
  const stoppedResponse = model.poll({ sourceId, token, request: poll({ pollSeq: "4", intentEpoch: "2", localDesired: "stopped" }), nowMs: 3_002 })
  assert.equal(stoppedResponse.body.action, null)
  assert.equal(model.status().controller.localDesired, "stopped")
  model.setPolicy({ maintenance: true })
  assert.equal(model.status().retry.lastResult, "cancelled")
  model.setPolicy({ maintenance: false, sourceEnabled: false })
  assert.equal(model.status().receiver.state, "source_disabled")
})

test("enforces 1/2/4 second backoff, budget three, and two-observation recovery", () => {
  const model = create(); armAndStall(model)
  const running = { intentEpoch: "2", localDesired: "running" }
  const first = model.poll({ sourceId, token, request: poll({ pollSeq: "3", ...running }), nowMs: 3_000 }).body.action
  model.poll({ sourceId, token, request: poll({ pollSeq: "4", ...running,
    lastAction: { commandId: first.commandId, result: "started" } }), nowMs: 3_100 })
  assert.equal(model.poll({ sourceId, token, request: poll({ pollSeq: "5", ...running }), nowMs: 4_000 }).body.action, null)
  const second = model.poll({ sourceId, token, request: poll({ pollSeq: "6", ...running }), nowMs: 4_100 }).body.action
  assert.equal(second.attempt, 2)
  model.poll({ sourceId, token, request: poll({ pollSeq: "7", ...running,
    lastAction: { commandId: second.commandId, result: "started" } }), nowMs: 4_200 })
  assert.equal(model.poll({ sourceId, token, request: poll({ pollSeq: "8", ...running }), nowMs: 6_100 }).body.action, null)
  const third = model.poll({ sourceId, token, request: poll({ pollSeq: "9", ...running }), nowMs: 6_200 }).body.action
  assert.equal(third.attempt, 3)
  model.poll({ sourceId, token, request: poll({ pollSeq: "10", ...running,
    lastAction: { commandId: third.commandId, result: "started" } }), nowMs: 6_300 })
  assert.equal(model.poll({ sourceId, token, request: poll({ pollSeq: "11", ...running }), nowMs: 10_200 }).body.action, null)
  assert.equal(model.poll({ sourceId, token, request: poll({ pollSeq: "12", ...running }), nowMs: 10_300 }).body.action, null)
  assert.equal(model.status().retry.lastResult, "exhausted")
  model.observeReceiver(receiver(1), 10_400); model.observeReceiver(receiver(2), 10_500); model.observeReceiver(receiver(3), 10_600)
  assert.equal(model.status().retry.attempts, 0)
  assert.equal(model.status().retry.lastResult, "recovered")
})

test("persists pending command and refences it to a restarted server run", () => {
  const first = create("server-a"); armAndStall(first)
  const action = first.poll({ sourceId, token, request: poll({ pollSeq: "3", intentEpoch: "2", localDesired: "running" }), nowMs: 3_000 }).body.action
  const restarted = create("server-b", first.persisted())
  const replay = restarted.poll({ sourceId, token, request: poll({ pollSeq: "3", intentEpoch: "2", localDesired: "running" }), nowMs: 3_100 }).body.action
  assert.equal(replay.commandId, action.commandId)
  assert.equal(replay.serverRunId, "server-b")
  restarted.expireLease(8_101)
  assert.equal(restarted.status().controller.online, false)
  assert.equal(restarted.status().receiver.state, "controller_offline")
})

function create(serverRunId = "server-a", persisted = null) {
  return new RecoveryModel({ sourceId, controllerId, token, serverRunId, persisted })
}

function stopped() { return poll({}) }

function poll(overrides) {
  return { schemaVersion: 1, controllerId, processEpoch: "1", pollSeq: "1", intentEpoch: "1",
    localDesired: "stopped", publisherState: "stopped", lastAction: null, ...overrides }
}

function receiver(value) {
  return { continuityGeneration: "1", videoFrames: value, audioBuffers: value, lastVideoPtsNs: value * 1_000_000 }
}

function stalled(model, now) {
  model.observeReceiver(receiver(0), 0); model.observeReceiver(receiver(0), now)
}

function armAndStall(model) {
  model.poll({ sourceId, token, request: stopped(), nowMs: 0 }); stalled(model, 3_000)
}
