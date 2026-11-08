import { describe, expect, it } from "vitest"
import { canManualRetry } from "./SourceRecoveryControls"

const sourceRecoveryFixture = {
  schemaVersion: 1 as const, sourceId: "source-a", policyVersion: "1", sourceEnabled: true,
  policy: { enabled: false, maintenance: false },
  controller: { enrolled: false, controllerId: null, online: false, processEpoch: null, localDesired: "stopped" as const },
  receiver: { state: "unknown" as const, continuityGeneration: null, videoFrames: null, audioBuffers: null, lastVideoPtsNs: null },
  retry: { attempts: 0, maximumAttempts: 3 as const, nextEligibleAtNs: null, lastResult: "never" as const },
  capabilities: { publisherRestart: true as const, processRestart: false as const, freezeDetection: "decoded_counter_and_pts_progress" as const },
}

describe("source recovery controls", () => {
  it("enables Retry only for an armed online running controller", () => {
    const eligible = {
      ...sourceRecoveryFixture,
      policy: { enabled: true, maintenance: false },
      controller: { enrolled: true, controllerId: "00000000-0000-4000-8000-000000000001", online: true, processEpoch: "1", localDesired: "running" as const },
      receiver: { ...sourceRecoveryFixture.receiver, state: "stalled" as const },
    }
    expect(canManualRetry(eligible, false)).toBe(true)
    expect(canManualRetry({ ...eligible, policy: { ...eligible.policy, maintenance: true } }, false)).toBe(false)
    expect(canManualRetry({ ...eligible, controller: { ...eligible.controller, online: false } }, false)).toBe(false)
    expect(canManualRetry({ ...eligible, controller: { ...eligible.controller, localDesired: "stopped" } }, false)).toBe(false)
    expect(canManualRetry({ ...eligible, sourceEnabled: false }, false)).toBe(false)
    expect(canManualRetry(eligible, true)).toBe(false)
  })

  it("does not treat enrollment or a healthy receiver as permission to start", () => {
    const stopped = {
      ...sourceRecoveryFixture,
      policy: { enabled: true, maintenance: false },
      controller: { enrolled: true, controllerId: "00000000-0000-4000-8000-000000000001", online: true, processEpoch: "1", localDesired: "stopped" as const },
      receiver: { ...sourceRecoveryFixture.receiver, state: "healthy" as const },
    }
    expect(canManualRetry(stopped, false)).toBe(false)
  })
})
