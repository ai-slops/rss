import assert from "node:assert/strict"
import test from "node:test"
import { assertDominantTone } from "./media-analysis.mjs"

test("favorable spectral ratios cannot turn numerical silence into fixture audio", () => {
  assert.throws(() => assertDominantTone({ 440: 1e-9, 880: 1e-12, noise: 1e-12, rms: 1e-9 }, 440, "quiet"))
})
test("audible intended tone passes while an audible wrong tone fails", () => {
  assert.doesNotThrow(() => assertDominantTone({ 440: 100, 880: 1, noise: 1, rms: 0.1 }, 440, "correct"))
  assert.throws(() => assertDominantTone({ 440: 1, 880: 100, noise: 1, rms: 0.1 }, 440, "wrong"))
})
