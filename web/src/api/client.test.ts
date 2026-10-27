import { describe, expect, it } from "vitest"
import { audioRouteSchema, commandResponseSchema, snapshotSchema, trackConnectionSchema } from "./schema"

describe("API boundary schemas", () => {
  it("accepts explicitly unknown native meters without inventing silence", () => {
    const parsed = audioRouteSchema.safeParse({ trackId: "mic", gainDb: 0, muted: false, routed: true, inputPeakDb: null, outputPeakDb: null, clipping: null })
    expect(parsed.success).toBe(true)
  })
  it("rejects a command response without an idempotency identity", () => {
    const parsed = commandResponseSchema.safeParse({ status: "applied", acceptedAt: "now" })
    expect(parsed.success).toBe(false)
  })

  it("rejects a snapshot that omits the engine run identity", () => {
    const parsed = snapshotSchema.safeParse({ cursor: "1", serverTime: "now" })
    expect(parsed.success).toBe(false)
  })

  it("accepts runtime source connection states and rejects the stale UI vocabulary", () => {
    expect(trackConnectionSchema.safeParse("connected").success).toBe(true)
    expect(trackConnectionSchema.safeParse("connecting").success).toBe(true)
    expect(trackConnectionSchema.safeParse("active").success).toBe(false)
  })
})
