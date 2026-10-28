import { resolve } from "node:path"

export function parseArgs(argv, root) {
  const values = new Map()
  const flags = new Set()
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index]
    if (token === "--dry-run") {
      flags.add(token)
      continue
    }
    if (!token.startsWith("--")) throw new Error(`Unexpected positional argument: ${token}`)
    const value = argv[index + 1]
    if (value === undefined || value.startsWith("--")) throw new Error(`Missing value for ${token}`)
    values.set(token, value)
    index += 1
  }
  const pluginDirectory = requiredPath(values, "--plugin-dir")
  const evidenceDirectory = requiredPath(values, "--evidence-dir")
  const deadlineSeconds = parsePositiveInteger(values.get("--deadline-seconds") ?? "300", "--deadline-seconds")
  return {
    pluginDirectory,
    evidenceDirectory,
    serverBinary: resolve(values.get("--server-binary") ?? `${root}/target/release/live-production`),
    supervisorBinary: resolve(values.get("--supervisor-binary") ?? `${root}/target/release/live-production-supervisor`),
    obsBinary: values.get("--obs-binary") ?? "obs",
    xvfbBinary: values.get("--xvfb-binary") ?? "Xvfb",
    webDist: resolve(values.get("--web-dist") ?? `${root}/web/dist`),
    publisherHost: values.get("--publisher-host") ?? "127.0.0.1",
    deadlineSeconds,
    preflightReport: values.get("--preflight-report") === undefined ? null : resolve(values.get("--preflight-report")),
    dryRun: flags.has("--dry-run"),
  }
}

export function usage() {
  return [
    "Usage: scripts/target/rehearsal.sh --plugin-dir STAGE --evidence-dir PRIVATE [options]",
    "  --server-binary PATH       Native Ubuntu daemon (default target/release/live-production)",
    "  --supervisor-binary PATH   Native Ubuntu supervisor",
    "  --obs-binary PATH          OBS Studio executable",
    "  --xvfb-binary PATH         Xvfb executable",
    "  --web-dist PATH            Built production web directory",
    "  --publisher-host HOST      SRT host visible to OBS (default 127.0.0.1)",
    "  --deadline-seconds N       Whole rehearsal deadline (default 300)",
    "  --preflight-report PATH    Require a passing preflight JSON",
    "  --dry-run                  Validate contracts without launching native processes",
  ].join("\n")
}

function requiredPath(values, name) {
  const value = values.get(name)
  if (value === undefined) throw new Error(`${name} is required`)
  return resolve(value)
}

function parsePositiveInteger(value, name) {
  const parsed = Number.parseInt(value, 10)
  if (!Number.isInteger(parsed) || parsed <= 0) throw new Error(`${name} must be a positive integer`)
  return parsed
}
