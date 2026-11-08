import assert from "node:assert/strict"
import test from "node:test"
import { chmod, mkdtemp, readFile, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { OwnedProcesses, waitForFile } from "./processes.mjs"

test("fails promptly when OBS exits before publishing its fixture report", async () => {
  const directory = await mkdtemp(join(tmpdir(), "lp-obs-exit-"))
  const record = { label: "obs", supervisor: { exitCode: 139, signalCode: null } }
  await assert.rejects(waitForFile(join(directory, "absent.json"), 500, record), (error) => {
    assert.equal(error.cause?.exitCode, 139)
    assert.equal(error.cause?.label, "obs")
    return true
  })
})

test("accepts an already published report even after its sender exits", async () => {
  const directory = await mkdtemp(join(tmpdir(), "lp-obs-report-"))
  const path = join(directory, "report.json")
  await writeFile(path, "{}", { mode: 0o600 })
  await waitForFile(path, 500, { label: "obs", supervisor: { exitCode: 0, signalCode: null } })
})

test("launches native commands only through the configured supervisor", async () => {
  const directory = await mkdtemp(join(tmpdir(), "lp-supervision-"))
  const argumentLog = join(directory, "arguments.txt")
  const supervisor = join(directory, "supervisor.sh")
  await writeFile(supervisor, `#!/bin/sh\nprintf '%s\\n' "$@" >"${argumentLog}"\nwhile [ "$1" != -- ]; do shift; done\nshift\n"$@" &\nwait $!\n`)
  await chmod(supervisor, 0o700)
  const processes = new OwnedProcesses(supervisor, { timeoutMs: 5_000, graceMs: 500 })
  const record = await processes.spawn("fixture", process.execPath, ["-e", "setTimeout(()=>{},5000)"], {
    cwd: directory,
    env: process.env,
    logPath: join(directory, "child.log"),
  })
  try {
    assert.notEqual(record.nativePid, record.supervisor.pid)
    const observed = await readFile(argumentLog, "utf8")
    assert.match(observed, /--timeout-ms\n5000\n--grace-ms\n500\n--/u)
    assert.match(observed, new RegExp(process.execPath.replaceAll("/", "\\/"), "u"))
  } finally {
    await processes.stopAll()
  }
  assert.equal(record.stopped, true)
})
