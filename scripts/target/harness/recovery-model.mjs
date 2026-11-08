import { randomUUID } from "node:crypto"

const backoffMs = [1_000, 2_000, 4_000]

export class RecoveryModel {
  #sourceId
  #controllerId
  #token
  #serverRunId
  #state

  constructor({ sourceId, controllerId, token, serverRunId = randomUUID(), persisted = null }) {
    this.#sourceId = sourceId
    this.#controllerId = controllerId
    this.#token = token
    this.#serverRunId = serverRunId
    this.#state = persisted ?? initialState()
    if (this.#state.pending !== null) this.#state.pending.serverRunId = serverRunId
  }

  setPolicy({ enabled = this.#state.enabled, maintenance = this.#state.maintenance,
    sourceEnabled = this.#state.sourceEnabled }) {
    Object.assign(this.#state, { enabled, maintenance, sourceEnabled })
    if (!enabled || maintenance || !sourceEnabled) this.#cancelPending()
  }

  observeReceiver(observation, nowMs) {
    const previous = this.#state.receiver
    const advanced = previous.videoFrames !== null
      && (observation.videoFrames > previous.videoFrames || observation.audioBuffers > previous.audioBuffers
        || observation.lastVideoPtsNs > previous.lastVideoPtsNs)
    if (previous.videoFrames === null || advanced) {
      this.#state.lastProgressAtMs = nowMs
      this.#state.healthyObservations = advanced ? this.#state.healthyObservations + 1 : 0
      this.#state.receiverState = advanced ? "healthy" : "warming"
    } else if (nowMs - this.#state.lastProgressAtMs >= 3_000) {
      this.#state.receiverState = "stalled"
      this.#state.healthyObservations = 0
    }
    this.#state.receiver = { ...observation }
    if (this.#state.healthyObservations >= 2 && this.#state.attempts > 0) {
      this.#state.attempts = 0
      this.#state.nextEligibleAtMs = null
      this.#state.lastResult = "recovered"
      this.#state.pending = null
    }
  }

  expireLease(nowMs) {
    if (this.#state.online && nowMs > this.#state.leaseUntilMs) {
      this.#state.online = false
      this.#state.receiverState = "controller_offline"
      this.#state.pending = null
    }
  }

  poll({ sourceId, token, request, nowMs }) {
    if (token === null) return { status: 401, body: null }
    if (sourceId !== this.#sourceId || token !== this.#token) return { status: 401, body: null }
    if (request.schemaVersion !== 1 || request.controllerId !== this.#controllerId) return { status: 409, body: null }
    const processEpoch = decimal(request.processEpoch)
    const pollSeq = decimal(request.pollSeq)
    const intentEpoch = decimal(request.intentEpoch)
    if (processEpoch < this.#state.processEpoch) return { status: 409, body: null }
    if (processEpoch > this.#state.processEpoch) {
      this.#startProcess(processEpoch, pollSeq, intentEpoch, nowMs)
      return { status: 200, body: this.#response(request, null) }
    }
    if (pollSeq < this.#state.pollSeq || intentEpoch < this.#state.intentEpoch) return { status: 409, body: null }
    if (pollSeq === this.#state.pollSeq) return { status: 200, body: this.#response(request, this.#authorizedPending()) }
    this.#state.pollSeq = pollSeq
    this.#state.online = true
    this.#state.leaseUntilMs = nowMs + 5_000
    this.#acknowledge(request.lastAction, nowMs)
    if (request.localDesired === "stopped") {
      this.#state.localDesired = "stopped"
      this.#state.armed = false
      this.#cancelPending()
    } else if (intentEpoch > this.#state.intentEpoch) {
      this.#state.intentEpoch = intentEpoch
      this.#state.localDesired = "running"
      this.#state.armed = true
    }
    let action = this.#authorizedPending()
    if (action === null && this.#eligible(nowMs)) action = this.#issue(nowMs)
    return { status: 200, body: this.#response(request, action) }
  }

  persisted() {
    return structuredClone(this.#state)
  }

  status() {
    const receiverState = !this.#state.sourceEnabled ? "source_disabled"
      : !this.#state.online ? "controller_offline" : this.#state.receiverState
    return {
      schemaVersion: 1, sourceId: this.#sourceId, policyVersion: "1", sourceEnabled: this.#state.sourceEnabled,
      policy: { enabled: this.#state.enabled, maintenance: this.#state.maintenance },
      controller: { enrolled: true, controllerId: this.#controllerId, online: this.#state.online,
        processEpoch: this.#state.processEpoch === 0n ? null : `${this.#state.processEpoch}`,
        localDesired: this.#state.localDesired },
      receiver: { state: receiverState, continuityGeneration: this.#state.receiver.continuityGeneration,
        videoFrames: decimalOrNull(this.#state.receiver.videoFrames), audioBuffers: decimalOrNull(this.#state.receiver.audioBuffers),
        lastVideoPtsNs: decimalOrNull(this.#state.receiver.lastVideoPtsNs) },
      retry: { attempts: this.#state.attempts, maximumAttempts: 3,
        nextEligibleAtNs: this.#state.nextEligibleAtMs === null ? null : `${this.#state.nextEligibleAtMs * 1_000_000}`,
        lastResult: this.#state.lastResult },
      capabilities: { publisherRestart: true, processRestart: false,
        freezeDetection: "decoded_counter_and_pts_progress" },
    }
  }

  #startProcess(processEpoch, pollSeq, intentEpoch, nowMs) {
    Object.assign(this.#state, { processEpoch, pollSeq, intentEpoch, localDesired: "stopped", armed: false,
      online: true, leaseUntilMs: nowMs + 5_000, pending: null })
  }

  #eligible(nowMs) {
    const next = this.#state.nextEligibleAtMs
    if (!this.#state.enabled || this.#state.maintenance || !this.#state.sourceEnabled
      || !this.#state.online || !this.#state.armed || this.#state.localDesired !== "running"
      || this.#state.receiverState !== "stalled" || (next !== null && nowMs < next)) return false
    if (this.#state.attempts >= 3) { this.#state.lastResult = "exhausted"; return false }
    return true
  }

  #issue(nowMs) {
    this.#state.attempts += 1
    this.#state.lastResult = "pending"
    this.#state.pending = { kind: "restartPublisher", commandId: randomUUID(), serverRunId: this.#serverRunId,
      controllerId: this.#controllerId, processEpoch: `${this.#state.processEpoch}`,
      intentEpoch: `${this.#state.intentEpoch}`, attempt: this.#state.attempts }
    this.#state.nextEligibleAtMs = nowMs + backoffMs[this.#state.attempts - 1]
    return this.#state.pending
  }

  #acknowledge(action, nowMs) {
    if (action === null || this.#state.pending === null || action.commandId !== this.#state.pending.commandId) return
    this.#state.lastResult = action.result
    this.#state.pending = null
    this.#state.nextEligibleAtMs = nowMs + backoffMs[Math.max(0, this.#state.attempts - 1)]
  }

  #authorizedPending() {
    return this.#state.enabled && !this.#state.maintenance && this.#state.sourceEnabled
      && this.#state.armed && this.#state.localDesired === "running" ? this.#state.pending : null
  }

  #cancelPending() {
    if (this.#state.pending !== null) this.#state.lastResult = "cancelled"
    this.#state.pending = null
  }

  #response(request, action) {
    return { ...this.status(), serverRunId: this.#serverRunId, processEpoch: request.processEpoch,
      pollSeq: request.pollSeq, intentEpoch: request.intentEpoch, nextPollMs: 1_000, action }
  }
}

function initialState() {
  return { enabled: true, maintenance: false, sourceEnabled: true, processEpoch: 0n, pollSeq: 0n,
    intentEpoch: 0n, localDesired: "stopped", armed: false, online: false, leaseUntilMs: 0,
    receiverState: "unknown", receiver: { continuityGeneration: null, videoFrames: null,
      audioBuffers: null, lastVideoPtsNs: null }, lastProgressAtMs: 0, healthyObservations: 0,
    attempts: 0, nextEligibleAtMs: null, lastResult: "never", pending: null }
}

function decimal(value) {
  if (typeof value !== "string" || !/^\d+$/u.test(value)) throw new Error("Recovery epochs and sequences must be decimal strings")
  return BigInt(value)
}

function decimalOrNull(value) {
  return value === null ? null : `${value}`
}
