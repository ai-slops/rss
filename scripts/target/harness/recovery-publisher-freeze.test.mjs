import assert from "node:assert/strict"
import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import test from "node:test"
import { writePrivateJson } from "./private-files.mjs"
import { runRequiredPublisherFreeze } from "./recovery-publisher-freeze.mjs"

async function fixtureFor(t, online = true) {
  const directory = await mkdtemp(join(tmpdir(), "publisher-freeze-"))
  t.after(() => rm(directory, { recursive: true, force: true }))
  const reportFile = join(directory, "report.json")
  await writePrivateJson(reportFile, { pollSeq: "1", restartExecutionCount: 0 })
  return {
    source: { id: "source" }, faults: { reportFile },
    bridge: { stats: () => ({ dataPaused: false, droppedData: 0, forwardedData: 10 }) },
    server: { api: { recoveryStatus: async () => ({
      receiver: { state: "stalled" }, controller: { online },
    }) } },
  }
}

test("an unsupported or failed writer freeze fails the required native gate", async t => {
  for (const result of ["publisherTransportNotIdentified", "publisherTransportNotQuiescent"]) {
    const fixture = await fixtureFor(t)
    const calls = []
    await assert.rejects(runRequiredPublisherFreeze(fixture, async kind => {
      calls.push(kind)
      return { executedCommandResult: kind === "freezePublisher" ? result : "paused" }
    }), /Publisher freeze hook failed/u)
    assert.deepEqual(calls, ["pauseActionDispatch", "freezePublisher"])
  }
})

test("a freeze acknowledgement cannot substitute for an online controller", async t => {
  const fixture = await fixtureFor(t, false)
  await assert.rejects(runRequiredPublisherFreeze(fixture, async kind => ({
    executedCommandResult: kind === "freezePublisher" ? "executed" : "paused",
  })), { code: "ERR_ASSERTION" })
})
