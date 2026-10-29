import assert from "node:assert/strict"
import { spawn } from "node:child_process"
import { chmod, lstat, mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import { createServer } from "node:http"
import { tmpdir } from "node:os"
import { join } from "node:path"
import test from "node:test"

const cli = new URL("./publisher-bundle.mjs", import.meta.url).pathname
const operatorPassword = "operator-password-private"
const scopedPassphrase = "scoped-publisher-passphrase-private"

test("writes one private existing-source OBS bundle without printing credentials", async (context) => {
  const directory = await mkdtemp(join(tmpdir(), "publisher-bundle-cli-"))
  context.after(() => rm(directory, { recursive: true, force: true }))
  const passwordFile = join(directory, "operator-password")
  const output = join(directory, "private", "camera.publisher.json")
  await writeFile(passwordFile, `${operatorPassword}\n`, { mode: 0o600 })
  const requests = []
  const server = createServer(async (request, response) => {
    const body = await readBody(request)
    requests.push({ method: request.method, url: request.url, cookie: request.headers.cookie, body })
    response.setHeader("content-type", "application/json")
    if (request.url === "/api/v1/auth/login") {
      response.setHeader("set-cookie", "live-production=test-session; HttpOnly")
      response.end("{}")
    } else if (request.url === "/api/v1/snapshot") {
      response.end(JSON.stringify(snapshot()))
    } else if (request.url === "/api/v1/sources/source-camera/publishing-endpoint") {
      response.setHeader("cache-control", "no-store, private")
      response.end(JSON.stringify(endpoint()))
    } else {
      response.statusCode = 404
      response.end("{}")
    }
  })
  await listen(server)
  context.after(() => close(server))
  const address = server.address()
  assert(address !== null && typeof address === "object")

  const result = await runCli([
    "--server-url", `http://127.0.0.1:${address.port}/api/v1`,
    "--password-file", passwordFile,
    "--source-id", "source-camera",
    "--obs-source-name", "Camera Scene",
    "--audio-mix-index", "2",
    "--target-host", "127.0.0.1",
    "--output", output,
  ])

  assert.equal(result.code, 0, result.stderr)
  const summary = JSON.parse(result.stdout)
  assert.deepEqual(summary, {
    status: "written", path: output, sourceId: "source-camera",
    obsSourceName: "Camera Scene", audioMixIndex: 2,
  })
  const bundle = JSON.parse(await readFile(output, "utf8"))
  assert.deepEqual(bundle.target, {
    sourceId: "source-camera", videoTrackId: "video-camera", audioTrackId: "audio-camera",
    host: "127.0.0.1", port: 12_345, passphrase: scopedPassphrase,
    streamId: "#!::u=source-camera,r=camera", bufferMs: 120,
  })
  assert.deepEqual(bundle.obs, { sourceName: "Camera Scene", audioMixIndex: 2 })
  assert.deepEqual(bundle.encoding, {
    width: 1280, height: 720, fpsNumerator: 30, fpsDenominator: 1,
    videoBitrateKbps: 2500, audioSampleRate: 48_000,
  })
  assert.equal((await lstat(output)).mode & 0o777, 0o600)
  assert.equal((await lstat(join(directory, "private"))).mode & 0o777, 0o700)
  assert.equal(`${result.stdout}${result.stderr}`.includes(operatorPassword), false)
  assert.equal(`${result.stdout}${result.stderr}`.includes(scopedPassphrase), false)
  assert.deepEqual(requests.map(({ method, url }) => [method, url]), [
    ["POST", "/api/v1/auth/login"], ["GET", "/api/v1/snapshot"],
    ["GET", "/api/v1/sources/source-camera/publishing-endpoint"],
  ])
  assert.deepEqual(JSON.parse(requests[0].body), { password: operatorPassword })
  assert.equal(requests[1].cookie, "live-production=test-session")
  assert.equal(requests.every(({ url }) => !url.includes(operatorPassword) && !url.includes(scopedPassphrase)), true)
})

test("rejects unsafe URL, missing remote target host, invalid mix and existing output", async (context) => {
  const directory = await mkdtemp(join(tmpdir(), "publisher-bundle-bad-input-"))
  context.after(() => rm(directory, { recursive: true, force: true }))
  const passwordFile = join(directory, "password")
  const output = join(directory, "bundle.json")
  await writeFile(passwordFile, operatorPassword, { mode: 0o600 })
  await writeFile(output, "existing", { mode: 0o600 })
  const identity = ["--password-file", passwordFile, "--source-id", "source-camera", "--obs-source-name", "Camera"]

  assert.match((await runCli(["--server-url", "http://example.com/api/v1", "--target-host", "ingest.example.com", ...identity, "--audio-mix-index", "0", "--output", join(directory, "fresh.json")])).stderr, /HTTPS/u)
  assert.match((await runCli(["--server-url", "https://control.example.com/api/v1", ...identity, "--audio-mix-index", "0", "--output", join(directory, "fresh.json")])).stderr, /target-host/u)
  assert.match((await runCli(["--server-url", "http://127.0.0.1:9/api/v1", "--target-host", "https://ingest.example.com", ...identity, "--audio-mix-index", "0", "--output", join(directory, "fresh.json")])).stderr, /host name or IP/u)
  assert.match((await runCli(["--server-url", "http://127.0.0.1:9/api/v1", "--target-host", "127.0.0.1", ...identity, "--audio-mix-index", "6", "--output", join(directory, "fresh.json")])).stderr, /audio-mix-index/u)
  assert.match((await runCli(["--server-url", "http://127.0.0.1:9/api/v1", "--target-host", "127.0.0.1", ...identity, "--audio-mix-index", "0", "--output", output])).stderr, /must not exist/u)
  assert.equal(await readFile(output, "utf8"), "existing")
})

test("rejects a password file readable by group or other users", async (context) => {
  const directory = await mkdtemp(join(tmpdir(), "publisher-bundle-permissions-"))
  context.after(() => rm(directory, { recursive: true, force: true }))
  const passwordFile = join(directory, "password")
  await writeFile(passwordFile, operatorPassword, { mode: 0o600 })
  await chmod(passwordFile, 0o644)
  const result = await runCli([
    "--server-url", "http://127.0.0.1:9/api/v1", "--password-file", passwordFile,
    "--source-id", "source-camera", "--obs-source-name", "Camera", "--audio-mix-index", "0",
    "--target-host", "127.0.0.1", "--output", join(directory, "bundle.json"),
  ])
  assert.notEqual(result.code, 0)
  assert.match(result.stderr, /owner-only/u)
  assert.equal(result.stderr.includes(operatorPassword), false)
})

test("preserves a bundle path created while authenticated discovery is in flight", async (context) => {
  const directory = await mkdtemp(join(tmpdir(), "publisher-bundle-race-"))
  context.after(() => rm(directory, { recursive: true, force: true }))
  const passwordFile = join(directory, "password")
  const output = join(directory, "camera.json")
  await writeFile(passwordFile, operatorPassword, { mode: 0o600 })
  const server = createServer(async (request, response) => {
    await readBody(request)
    response.setHeader("content-type", "application/json")
    if (request.url === "/api/v1/auth/login") {
      response.setHeader("set-cookie", "live-production=test-session; HttpOnly")
      response.end("{}")
    } else if (request.url === "/api/v1/snapshot") {
      response.end(JSON.stringify(snapshot()))
    } else {
      await writeFile(output, "concurrent-owner", { flag: "wx", mode: 0o600 })
      response.end(JSON.stringify(endpoint()))
    }
  })
  await listen(server)
  context.after(() => close(server))
  const address = server.address()
  assert(address !== null && typeof address === "object")
  const result = await runCli([
    "--server-url", `http://127.0.0.1:${address.port}/api/v1`,
    "--password-file", passwordFile, "--source-id", "source-camera",
    "--obs-source-name", "Camera", "--audio-mix-index", "0", "--output", output,
  ])
  assert.notEqual(result.code, 0)
  assert.equal(await readFile(output, "utf8"), "concurrent-owner")
  assert.equal(`${result.stdout}${result.stderr}`.includes(scopedPassphrase), false)
})

function snapshot() {
  return { project: { sources: [{
    id: "source-camera", name: "Camera Input", enabled: true,
    adapter: { kind: "srt", bindAddress: "0.0.0.0", port: 12_345, bufferMs: 120 },
    tracks: [{ id: "video-camera", kind: "video" }, { id: "audio-camera", kind: "audio" }],
  }] } }
}

function endpoint() {
  return {
    sourceId: "source-camera", protocol: "srt", bindAddress: "0.0.0.0", port: 12_345,
    bufferMs: 120, passphrase: scopedPassphrase, streamId: "#!::u=source-camera,r=camera", pbkeylen: 16,
  }
}

async function runCli(args) {
  const child = spawn(process.execPath, [cli, ...args], { stdio: ["ignore", "pipe", "pipe"] })
  const stdout = []; const stderr = []
  child.stdout.on("data", (chunk) => stdout.push(chunk))
  child.stderr.on("data", (chunk) => stderr.push(chunk))
  const code = await new Promise((resolve, reject) => {
    child.once("error", reject)
    child.once("exit", resolve)
  })
  return { code, stdout: Buffer.concat(stdout).toString("utf8").trim(), stderr: Buffer.concat(stderr).toString("utf8").trim() }
}

function listen(server) {
  return new Promise((resolve, reject) => {
    server.once("error", reject)
    server.listen(0, "127.0.0.1", resolve)
  })
}

function close(server) {
  return new Promise((resolve, reject) => server.close((error) => error === undefined ? resolve() : reject(error)))
}

async function readBody(request) {
  const chunks = []
  for await (const chunk of request) chunks.push(chunk)
  return Buffer.concat(chunks).toString("utf8")
}
