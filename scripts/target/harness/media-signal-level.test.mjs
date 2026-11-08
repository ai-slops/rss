import assert from "node:assert/strict"
import test from "node:test"
import { assertDominantTone, assertProgramTones } from "./media-analysis.mjs"

test("favorable spectral ratios cannot turn numerical silence into fixture audio", () => {
  assert.throws(() => assertDominantTone({ 440: 1e-9, 880: 1e-12, noise: 1e-12, rms: 1e-9 }, 440, "quiet"))
})
test("audible intended tone passes while an audible wrong tone fails", () => {
  assert.doesNotThrow(() => assertDominantTone({ 440: 10000, 880: 1, noise: 1, rms: 0.1, sampleCount: 1000 }, 440, "correct"))
  assert.throws(() => assertDominantTone({ 440: 1, 880: 10000, noise: 1, rms: 0.1, sampleCount: 1000 }, 440, "wrong"))
})
test("one loud Program tone cannot hide a missing second routed tone", () => {
  assert.throws(() => assertProgramTones({ 440: 10000, 880: 1e-9, noise: 1e-12, rms: 0.1, sampleCount: 1000 }, [440, 880]))
  assert.doesNotThrow(() => assertProgramTones({ 440: 10000, 880: 10000, noise: 1, rms: 0.2, sampleCount: 1000 }, [440, 880]))
})
