import assert from "node:assert/strict"
import test from "node:test"
import { assertNativeBackoff } from "./recovery-native-safety.mjs"

test("native backoff requires the actual bounded delay rather than only a maximum", () => {
  assert.doesNotThrow(() => assertNativeBackoff(1, 900000000n))
  assert.doesNotThrow(() => assertNativeBackoff(2, 1900000000n))
  assert.doesNotThrow(() => assertNativeBackoff(3, 3980000000n))
  assert.throws(() => assertNativeBackoff(1, 1000000n))
  assert.throws(() => assertNativeBackoff(3, 2000000000n))
  assert.throws(() => assertNativeBackoff(3, 5000000000n))
})
