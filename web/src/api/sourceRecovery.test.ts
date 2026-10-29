import { describe, expect, it } from "vitest"
import { sourceRecoveryStatusSchema } from "./sourceRecovery"

const status = {
  schemaVersion: 1,
  sourceId: "source-a",
  policyVersion: "1",
  sourceEnabled: true,
  policy: { enabled: false, maintenance: false },
  controller: { enrolled: false, controllerId: null, online: false, processEpoch: null, localDesired: "stopped" },
  receiver: { state: "unknown", continuityGeneration: null, videoFrames: null, audioBuffers: null, lastVideoPtsNs: null },
  retry: { attempts: 0, maximumAttempts: 3, nextEligibleAtNs: null, lastResult: "never" },
  capabilities: { publisherRestart: true, processRestart: false, freezeDetection: "decoded_counter_and_pts_progress" },
}

describe("source recovery boundary", () => {
  it("parses the fixed status shape and nullable decimal-string observations", () => {
    expect(sourceRecoveryStatusSchema.parse(status)).toEqual(status)
    expect(sourceRecoveryStatusSchema.parse({ ...status, receiver: { ...status.receiver, continuityGeneration: "12", videoFrames: "1200", audioBuffers: "2400", lastVideoPtsNs: "9007199254740993" } }).receiver.lastVideoPtsNs).toBe("9007199254740993")
  })

  it("rejects numeric counters and unknown receiver states", () => {
    expect(sourceRecoveryStatusSchema.safeParse({ ...status, receiver: { ...status.receiver, videoFrames: 1 } }).success).toBe(false)
    expect(sourceRecoveryStatusSchema.safeParse({ ...status, receiver: { ...status.receiver, state: "live" } }).success).toBe(false)
  })

  it("rejects secret or uncontracted response fields", () => {
    expect(sourceRecoveryStatusSchema.safeParse({ ...status, controllerToken: "secret" }).success).toBe(false)
  })
})
