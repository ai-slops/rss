import assert from "node:assert/strict"
import { lstat, mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import test from "node:test"
import { recoveryFaultKinds, validateFaultFixture, writeRecoveryFaultCommand, writeRecoveryFaultFixture } from "./recovery-faults.mjs"

test("writes one private source-only finite recovery fault fixture", async (context) => {
  const directory = await mkdtemp(join(tmpdir(), "recovery-faults-"))
  context.after(() => rm(directory, { recursive: true, force: true }))
  const bundle = join(directory, "source.publisher.json")
  await writeFile(bundle, "{}", { mode: 0o600 })
  const output = await writeRecoveryFaultFixture(directory, {
    sourceId: "source-a", publisherBundle: bundle, deadlineSeconds: 120,
  })
  const fixture = JSON.parse(await readFile(output.fixtureFile, "utf8"))
  assert.equal(validateFaultFixture(fixture, "source-a"), fixture)
  assert.deepEqual(fixture.operations.map(({ kind }) => kind), recoveryFaultKinds)
  assert.equal(fixture.operations.some((entry) => "command" in entry || "url" in entry), false)
  assert.equal((await lstat(output.fixtureFile)).mode & 0o777, 0o600)
  assert.deepEqual(await writeRecoveryFaultCommand(output.commandFile, "1", "dropPublisher"),
    { schemaVersion: 1, commandId: "1", kind: "dropPublisher" })
  assert.deepEqual(await writeRecoveryFaultCommand(output.commandFile, "2", "freezePublisher", 4_000),
    { schemaVersion: 1, commandId: "2", kind: "freezePublisher", durationMs: 4_000 })
  assert.equal((await lstat(output.commandFile)).mode & 0o777, 0o600)
})
