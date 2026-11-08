import assert from "node:assert/strict"
import test from "node:test"
import { publicCrashRecovery } from "./public-report.mjs"

test("crash recovery report excludes API process state and credentials", () => {
  const crash = {
    outputId: "output-a",
    rawFragments: 3,
    confirmedClosedBeforeKill: 2,
    decodedAfterRestart: 2,
    recoveries: ["partial", "recovered"],
    hashesPreserved: true,
    restarted: { api: { password: "secret" }, record: {} },
  }
  crash.restarted.record.circular = crash
  const value = publicCrashRecovery(crash)
  assert.deepEqual(value, {
    outputId: "output-a",
    rawFragments: 3,
    confirmedClosedBeforeKill: 2,
    decodedAfterRestart: 2,
    recoveries: ["partial", "recovered"],
    hashesPreserved: true,
  })
  assert.doesNotThrow(() => JSON.stringify(value))
  assert.equal(JSON.stringify(value).includes("secret"), false)
})
