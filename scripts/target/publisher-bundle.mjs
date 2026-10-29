#!/usr/bin/env node
import { constants } from "node:fs"
import { lstat, open, unlink } from "node:fs/promises"
import { isIP } from "node:net"
import { dirname, resolve } from "node:path"
import { ApiClient } from "./harness/api-client.mjs"
import { createPublisherBundle } from "./harness/publisher-bundle.mjs"
import { ensurePrivateDirectory } from "./harness/private-files.mjs"

const defaults = {
  width: 1280,
  height: 720,
  fpsNumerator: 30,
  fpsDenominator: 1,
  videoBitrateKbps: 2500,
}

try {
  await main(process.argv.slice(2))
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : "Publisher bundle generation failed"}\n`)
  process.exitCode = 1
}

async function main(argv) {
  if (argv.length === 1 && argv[0] === "--help") {
    process.stdout.write(`${usage()}\n`)
    return
  }
  const { values, flags } = parseArgs(argv)
  const withRecovery = flags.has("--with-recovery")
  const rotateRecovery = flags.has("--rotate-recovery")
  if (rotateRecovery && !withRecovery) throw new Error("--rotate-recovery requires --with-recovery")
  const control = parseControlUrl(required(values, "--server-url"))
  const output = resolve(required(values, "--output"))
  await requireAbsent(output)
  const password = await readPrivatePassword(resolve(required(values, "--password-file")))
  const sourceId = required(values, "--source-id")
  const obsSourceName = required(values, "--obs-source-name")
  const audioMixIndex = integer(values, "--audio-mix-index")
  if (audioMixIndex < 0 || audioMixIndex > 5) throw new Error("--audio-mix-index must be an integer from 0 through 5")
  const targetHost = targetHostFor(values.get("--target-host"), control)
  const encoding = {
    width: optionalInteger(values, "--width", defaults.width),
    height: optionalInteger(values, "--height", defaults.height),
    fpsNumerator: optionalInteger(values, "--fps-numerator", defaults.fpsNumerator),
    fpsDenominator: optionalInteger(values, "--fps-denominator", defaults.fpsDenominator),
    videoBitrateKbps: optionalInteger(values, "--video-bitrate-kbps", defaults.videoBitrateKbps),
  }

  const api = new ApiClient(control.baseUrl)
  await api.login(password)
  const snapshot = await api.snapshot()
  const source = snapshot.project?.sources?.find((candidate) => candidate.id === sourceId)
  if (source === undefined) throw new Error(`Existing source ${sourceId} is absent from the authenticated snapshot`)
  if (source.adapter?.kind !== "srt") throw new Error(`Existing source ${sourceId} is not an SRT input`)
  const endpoint = await api.publishingEndpoint(sourceId)
  const bundle = createPublisherBundle({
    snapshot,
    endpoint,
    host: targetHost,
    sourceName: obsSourceName,
    audioMixIndex,
    encoding,
  })
  const file = withRecovery ? await reserveOutput(output) : null
  try {
    if (withRecovery) bundle.recovery = recoveryBundle(await api.enrollRecovery(sourceId, rotateRecovery), control, sourceId)
    await writeBundle(output, bundle, file)
  } catch (error) {
    if (file !== null) { await file.close().catch(() => undefined); await unlink(output).catch(() => undefined) }
    throw error
  }
  process.stdout.write(`${JSON.stringify({
    status: "written",
    path: output,
    sourceId: bundle.target.sourceId,
    obsSourceName: bundle.obs.sourceName,
    audioMixIndex: bundle.obs.audioMixIndex,
  })}\n`)
}

function parseArgs(argv) {
  const booleanFlags = new Set(["--with-recovery", "--rotate-recovery"])
  const allowed = new Set([
    "--server-url", "--password-file", "--source-id", "--obs-source-name", "--audio-mix-index",
    "--target-host", "--output", "--width", "--height", "--fps-numerator", "--fps-denominator",
    "--video-bitrate-kbps",
  ])
  const values = new Map(); const flags = new Set()
  for (let index = 0; index < argv.length;) {
    const name = argv[index]
    if (booleanFlags.has(name)) {
      if (flags.has(name)) throw new Error(`Duplicate argument: ${name}`)
      flags.add(name); index += 1; continue
    }
    const value = argv[index + 1]
    if (!allowed.has(name)) throw new Error(`Unknown argument: ${name ?? "<missing>"}\n${usage()}`)
    if (value === undefined || value.startsWith("--")) throw new Error(`Missing value for ${name}\n${usage()}`)
    if (values.has(name)) throw new Error(`Duplicate argument: ${name}`)
    values.set(name, value)
    index += 2
  }
  return { values, flags }
}

function required(values, name) {
  const value = values.get(name)
  if (value === undefined || value.length === 0) throw new Error(`${name} is required\n${usage()}`)
  return value
}

function integer(values, name) {
  const value = required(values, name)
  if (!/^\d+$/u.test(value)) throw new Error(`${name} must be an integer`)
  return Number.parseInt(value, 10)
}

function optionalInteger(values, name, fallback) {
  if (!values.has(name)) return fallback
  const value = integer(values, name)
  if (value <= 0) throw new Error(`${name} must be a positive integer`)
  return value
}

function parseControlUrl(value) {
  let url
  try {
    url = new URL(value)
  } catch {
    throw new Error("--server-url must be a valid URL")
  }
  const loopback = isLoopback(url.hostname)
  if (url.protocol !== "https:" && !(url.protocol === "http:" && loopback)) {
    throw new Error("--server-url must use HTTPS except for a loopback server")
  }
  if (url.username !== "" || url.password !== "" || url.search !== "" || url.hash !== "") {
    throw new Error("--server-url must not contain credentials, query parameters, or a fragment")
  }
  if (url.pathname.replace(/\/+$/u, "") !== "/api/v1") {
    throw new Error("--server-url must end with /api/v1")
  }
  return { baseUrl: `${url.origin}/api/v1`, origin: url.origin, hostname: unbracket(url.hostname), loopback }
}

function recoveryBundle(enrollment, control, sourceId) {
  const expectedPath = `/api/v1/source-recovery/${encodeURIComponent(sourceId)}/poll`
  if (enrollment?.schemaVersion !== 1 || enrollment.sourceId !== sourceId
    || enrollment.controlPath !== expectedPath || !isUuid(enrollment.controllerId)
    || typeof enrollment.controllerToken !== "string" || enrollment.controllerToken.length < 16
    || typeof enrollment.policyVersion !== "string" || enrollment.policyVersion.length === 0) {
    throw new Error("Recovery enrollment returned an invalid source-scoped controlPath or credential")
  }
  return { schemaVersion: 1, controlUrl: `${control.origin}${expectedPath}`,
    controllerId: enrollment.controllerId, controllerToken: enrollment.controllerToken }
}

function isUuid(value) {
  return typeof value === "string" && /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/iu.test(value)
}

async function reserveOutput(path) {
  await ensurePrivateDirectory(dirname(path))
  return open(path, "wx", 0o600)
}

async function writeBundle(path, bundle, reserved) {
  const file = reserved ?? await reserveOutput(path)
  try { await file.writeFile(`${JSON.stringify(bundle, null, 2)}\n`) } finally { await file.close() }
}

function targetHostFor(value, control) {
  if (value === undefined) {
    if (!control.loopback) throw new Error("--target-host is required for a remote server")
    return control.hostname
  }
  const unbracketed = unbracket(value)
  if (value.length === 0 || /[/ ?&#@\t\r\n]/u.test(value) || (value.includes(":") && isIP(unbracketed) !== 6)) {
    throw new Error("--target-host must be a host name or IP address without a scheme, path, or port")
  }
  return unbracketed
}

async function readPrivatePassword(path) {
  const file = await open(path, constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK)
  try {
    const stat = await file.stat()
    if (!stat.isFile() || stat.size > 4096) throw new Error("--password-file must be a regular file of at most 4096 bytes")
    if ((stat.mode & 0o077) !== 0) throw new Error("--password-file must be owner-only")
    if (typeof process.getuid === "function" && stat.uid !== process.getuid()) {
      throw new Error("--password-file must be owned by the current user")
    }
    const bytes = Buffer.alloc(4097)
    let length = 0
    while (length < bytes.length) {
      const { bytesRead } = await file.read(bytes, length, bytes.length - length, null)
      if (bytesRead === 0) break
      length += bytesRead
    }
    if (length > 4096) throw new Error("--password-file exceeds 4096 bytes")
    const password = bytes.subarray(0, length).toString("utf8").replace(/\r?\n$/u, "")
    if (password.length === 0 || /[\r\n]/u.test(password)) throw new Error("--password-file must contain exactly one non-empty line")
    return password
  } finally {
    await file.close()
  }
}

async function requireAbsent(path) {
  try {
    await lstat(path)
  } catch (error) {
    if (error.code === "ENOENT") return
    throw error
  }
  throw new Error("--output must not exist")
}

function isLoopback(hostname) {
  return ["localhost", "127.0.0.1", "::1", "[::1]"].includes(hostname.toLowerCase())
}

function unbracket(hostname) {
  return hostname.startsWith("[") && hostname.endsWith("]") ? hostname.slice(1, -1) : hostname
}

function usage() {
  return [
    "Usage: node scripts/target/publisher-bundle.mjs --server-url URL --password-file FILE",
    "  --source-id ID --obs-source-name NAME --audio-mix-index 0..5 --output NEW_FILE",
    "  [--target-host HOST] [--width 1280] [--height 720] [--fps-numerator 30]",
    "  [--fps-denominator 1] [--video-bitrate-kbps 2500] [--with-recovery] [--rotate-recovery]",
  ].join("\n")
}
