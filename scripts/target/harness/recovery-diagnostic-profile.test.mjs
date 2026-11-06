import assert from "node:assert/strict"
import test from "node:test"
import { diagnosticOmissions, recoveryDisposition } from "./recovery-diagnostic-profile.mjs"

test("default native profile requires every case for acceptance", () => {
  assert.deepEqual(recoveryDisposition(diagnosticOmissions()), { status: "pass", obsClaim: true, diagnosticOmissions: [], requiredUnavailable: [] })
})
test("explicit diagnostic omission cannot become a native acceptance pass", () => {
  const result = recoveryDisposition(diagnosticOmissions("freezePublisher,obsProcessSigstop"))
  assert.equal(result.status, "partial")
  assert.equal(result.obsClaim, false)
  assert.deepEqual(result.diagnosticOmissions, ["freezePublisher", "obsProcessSigstop"])
})
test("unknown and duplicate omissions fail closed", () => {
  assert.throws(() => diagnosticOmissions("authorization"))
  assert.throws(() => diagnosticOmissions("freezePublisher,freezePublisher"))
})
test("an unavailable publisher-freeze hook prevents full acceptance", () => {
  const report = recoveryDisposition([], ["publisherTransportFreeze"])
  assert.equal(report.status, "partial")
  assert.equal(report.obsClaim, false)
  assert.deepEqual(report.requiredUnavailable, ["publisherTransportFreeze"])
})
