import assert from "node:assert/strict"
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { spawn } from "node:child_process"
import test from "node:test"

const entry = new URL("./recovery-native.mjs", import.meta.url).pathname

test("fails precisely when the plugin contract lacks ack-driven proof fields", async (context) => {
  const directory = await mkdtemp(join(tmpdir(), "recovery-native-contract-"))
  context.after(() => rm(directory, { recursive: true, force: true }))
  const contract = join(directory, "RECOVERY.md")
  const evidence = join(directory, "evidence")
  await writeFile(contract, "timer-only recovery fixture", { mode: 0o600 })
  const result = await run(argumentsFor(directory, evidence, ["--plugin-recovery-contract", contract]))
  assert.equal(result.code, 1)
  const report = JSON.parse(await readFile(join(evidence, "recovery-report.json"), "utf8"))
  assert.equal(report.reason, "pluginRecoveryCommandInterfaceUnavailable")
  assert.deepEqual(report.missingFields,
    ["commandFile", "restartExecutionCount", "executedCommandId", "executedCommandResult",
      "recoveryActionPending", "pendingRecoveryCommandId", "duplicateCommandId",
      "pauseActionDispatch", "resumeActionDispatch"])
  assert.equal(report.obsClaim, false)
})

test("recognizes the actual plugin contract and reports only missing native runtime", async (context) => {
  const directory = await mkdtemp(join(tmpdir(), "recovery-native-prerequisite-"))
  context.after(() => rm(directory, { recursive: true, force: true }))
  const evidence = join(directory, "evidence")
  const result = await run(argumentsFor(directory, evidence))
  assert.equal(result.code, 1)
  assert.match(result.stderr, /Native recovery prerequisite missing/u)
  assert.doesNotMatch(result.stderr, /pluginRecoveryCommandInterfaceUnavailable/u)
})

function argumentsFor(directory, evidence, extra = []) {
  return ["--plugin-dir", directory, "--evidence-dir", evidence,
    "--server-binary", join(directory, "missing-server"),
    "--supervisor-binary", join(directory, "missing-supervisor"),
    "--web-dist", directory, "--obs-binary", "missing-obs", "--xvfb-binary", "missing-xvfb", ...extra]
}

async function run(args) {
  const child = spawn(process.execPath, [entry, ...args], { stdio: ["ignore", "pipe", "pipe"] })
  const stdout = []; const stderr = []
  child.stdout.on("data", (chunk) => stdout.push(chunk)); child.stderr.on("data", (chunk) => stderr.push(chunk))
  const code = await new Promise((resolve, reject) => { child.once("error", reject); child.once("exit", resolve) })
  return { code, stdout: Buffer.concat(stdout).toString("utf8"), stderr: Buffer.concat(stderr).toString("utf8") }
}
