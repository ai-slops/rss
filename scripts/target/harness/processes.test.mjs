import assert from "node:assert/strict"
import test from "node:test"
import { chmod, mkdtemp, readFile, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { OwnedProcesses } from "./processes.mjs"

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
