import { describe, expect, it } from "vitest"
import { commandResponseSchema, snapshotSchema } from "./schema"

describe("API boundary schemas", () => {
  it("rejects a command response without an idempotency identity", () => {
    const parsed = commandResponseSchema.safeParse({ status: "applied", acceptedAt: "now" })
    expect(parsed.success).toBe(false)
  })

  it("rejects a snapshot that omits the engine run identity", () => {
    const parsed = snapshotSchema.safeParse({ cursor: "1", serverTime: "now" })
    expect(parsed.success).toBe(false)
  })
})
