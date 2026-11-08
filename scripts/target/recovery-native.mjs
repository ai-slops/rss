#!/usr/bin/env node
import { constants } from "node:fs"
import { access, mkdir, readFile } from "node:fs/promises"
import { dirname, resolve } from "node:path"
import { runRecoveryNative } from "./harness/recovery-native-flow.mjs"
import { writePrivateJson } from "./harness/private-files.mjs"
import { runPreflight } from "./harness/preflight.mjs"

const root = resolve(new URL("../..", import.meta.url).pathname)
let failureEvidence = null

try {
  const values = parseArgs(process.argv.slice(2))
  const evidenceDirectory = resolve(required(values, "--evidence-dir"))
  await mkdir(evidenceDirectory, { mode: 0o700 })
  failureEvidence = evidenceDirectory
  const privateDirectory = resolve(evidenceDirectory, "private")
  await mkdir(privateDirectory, { mode: 0o700 })
  const contractPath = resolve(values.get("--plugin-recovery-contract")
    ?? resolve(root, "integrations/obs-live-production/RECOVERY.md"))
  const contract = await readFile(contractPath, "utf8")
  const fixtureSource = await readFile(resolve(dirname(contractPath), "src/recovery_fixture.cpp"), "utf8").catch(() => "")
  const missingFields = missingContractFields(`${contract}\n${fixtureSource}`)
  if (missingFields.length > 0) {
    await failReport(evidenceDirectory, {
      reason: "pluginRecoveryCommandInterfaceUnavailable", pluginContract: contractPath, missingFields,
    })
    process.exitCode = 1
  } else {
    const options = {
      root,
      evidenceDirectory,
      privateDirectory,
      pluginDirectory: resolve(required(values, "--plugin-dir")),
      serverBinary: resolve(required(values, "--server-binary")),
      supervisorBinary: resolve(required(values, "--supervisor-binary")),
      webDist: resolve(required(values, "--web-dist")),
      obsBinary: resolve(required(values, "--obs-binary")),
      xvfbBinary: resolve(required(values, "--xvfb-binary")),
      deadlineSeconds: positive(values.get("--deadline-seconds") ?? "180", "--deadline-seconds"),
    }
    await nativePrerequisites(options)
    const preflight = await runPreflight({ ...options,
      reportPath: resolve(privateDirectory, "native-preflight.json") })
    if (preflight.status !== "pass") throw new Error("Native Ubuntu recovery preflight did not pass")
    const report = await runRecoveryNative(options, evidenceDirectory, privateDirectory)
    if (report.status !== "pass") process.exitCode = 1
    process.stdout.write(`${JSON.stringify({ status: report.status,
      report: resolve(evidenceDirectory, "recovery-report.json") })}\n`)
  }
} catch (error) {
  const message = error instanceof Error ? error.message : "Recovery native acceptance failed"
  if (failureEvidence !== null) {
    await writePrivateJson(resolve(failureEvidence, "recovery-report.json"), {
      schemaVersion: 1, status: "failed",
      nativeExecution: error instanceof Error && error.cause !== null &&
        typeof error.cause === "object" && error.cause.nativeExecution === true,
      obsClaim: false,
      reason: "nativePrerequisiteOrExecutionFailure", detail: message,
    }).catch(() => undefined)
  }
  process.stderr.write(`${message}\n`)
  process.exitCode = 1
}

function parseArgs(argv) {
  const allowed = new Set(["--plugin-dir", "--evidence-dir", "--server-binary", "--supervisor-binary",
    "--web-dist", "--obs-binary", "--xvfb-binary", "--deadline-seconds", "--plugin-recovery-contract"])
  const values = new Map()
  for (let index = 0; index < argv.length; index += 2) {
    const name = argv[index]; const value = argv[index + 1]
    if (!allowed.has(name) || value === undefined || value.startsWith("--")) throw new Error(usage())
    if (values.has(name)) throw new Error(`Duplicate argument: ${name}`)
    values.set(name, value)
  }
  for (const name of ["--plugin-dir", "--evidence-dir", "--server-binary", "--supervisor-binary",
    "--web-dist", "--obs-binary", "--xvfb-binary"]) required(values, name)
  return values
}

function required(values, name) {
  const value = values.get(name)
  if (value === undefined || value.length === 0) throw new Error(usage())
  return value
}

function missingContractFields(contract) {
  return ["commandFile", "restartExecutionCount", "executedCommandId", "executedCommandResult",
    "recoveryActionPending", "pendingRecoveryCommandId", "duplicateCommandId",
    "pauseActionDispatch", "resumeActionDispatch"]
    .filter((field) => !contract.includes(field))
}

async function nativePrerequisites(options) {
  try {
    await access(options.serverBinary, constants.X_OK)
    await access(options.supervisorBinary, constants.X_OK)
    await access(options.pluginDirectory)
    await access(options.webDist)
    await access(options.obsBinary, constants.X_OK)
    await access(options.xvfbBinary, constants.X_OK)
  } catch (error) {
    throw new Error(`Native recovery prerequisite missing: ${error.path ?? "unknown path"}`)
  }
}

async function failReport(evidenceDirectory, fields) {
  const report = { schemaVersion: 1, status: "failed", nativeExecution: false, obsClaim: false,
    requiredEvidence: ["actual OBS source events", "decoded receiver video/audio progress", "finite fault results"],
    ...fields }
  await writePrivateJson(resolve(evidenceDirectory, "recovery-report.json"), report)
  process.stdout.write(`${JSON.stringify({ status: report.status,
    report: resolve(evidenceDirectory, "recovery-report.json") })}\n`)
}

function positive(value, name) {
  if (!/^\d+$/u.test(value) || Number.parseInt(value, 10) <= 0) throw new Error(`${name} must be a positive integer`)
  return Number.parseInt(value, 10)
}

function usage() {
  return "Usage: node scripts/target/recovery-native.mjs --plugin-dir STAGE --evidence-dir NEW --server-binary PATH --supervisor-binary PATH --web-dist PATH --obs-binary PATH --xvfb-binary PATH [--deadline-seconds 180]"
}
