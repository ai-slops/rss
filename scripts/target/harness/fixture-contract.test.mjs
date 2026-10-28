import assert from "node:assert/strict"
import test from "node:test"
import { mkdtemp, readFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { validateFixtureReport, writeFixtureFiles } from "./fixture-contract.mjs"
import { writePrivateJson } from "./private-files.mjs"

test("writes exact local fixture environment contract", async () => {
  const directory = await mkdtemp(join(tmpdir(), "lp-fixture-"))
  const fixture = await writeFixtureFiles(directory, ["/private/red.publisher.json"], [{
    sourceId: "red", color: "red", frameCounter: true, toneHz: 440, audioMixIndex: 0,
  }], 120)
  const value = JSON.parse(await readFile(fixture.fixtureFile, "utf8"))
  assert.deepEqual(value, {
    schemaVersion: 1,
    localFixtureOnly: true,
    senders: ["/private/red.publisher.json"],
    reportFile: fixture.reportFile,
    controlFile: fixture.controlFile,
    deadlineSeconds: 120,
  })
})

test("requires actual module, sender, and negative-case evidence", async () => {
  const directory = await mkdtemp(join(tmpdir(), "lp-report-"))
  const path = join(directory, "report.json")
  const bundle = { target: { sourceId: "red" }, obs: { audioMixIndex: 0 } }
  await writePrivateJson(path, {
    schemaVersion: 1,
    moduleLoaded: true,
    module: "obs-live-production",
    state: "ready",
    senders: [{ sourceId: "red", audioMixIndex: 0, state: "sending" }],
    negativeCases: [
      { kind: "badAuth", result: "rejected" },
      { kind: "duplicateSender", result: "rejected" },
    ],
  })
  assert.equal((await validateFixtureReport(path, [bundle])).moduleLoaded, true)
})
