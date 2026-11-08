import assert from "node:assert/strict"
import { spawn } from "node:child_process"
import { createServer } from "node:http"
import { mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import test from "node:test"

const cli = new URL("./publisher-bundle.mjs", import.meta.url).pathname
const password = "private-operator-password"
const controllerToken = "source-only-controller-token"

test("--with-recovery enrolls once and writes only the source-scoped controller credential", async (context) => {
  const fixture = await setup(context, enrollment())
  const result = await runCli([...fixture.args, "--with-recovery"])
  assert.equal(result.code, 0, result.stderr)
  const bundle = JSON.parse(await readFile(fixture.output, "utf8"))
  assert.deepEqual(bundle.recovery, {
    schemaVersion: 1,
    controlUrl: `${fixture.origin}/api/v1/source-recovery/source-camera/poll`,
    controllerId: "11111111-1111-4111-8111-111111111111",
    controllerToken,
  })
  assert.equal(JSON.stringify(bundle).includes(password), false)
  assert.equal(`${result.stdout}${result.stderr}`.includes(controllerToken), false)
  assert.equal((await stat(fixture.output)).mode & 0o777, 0o600)
  assert.deepEqual(fixture.requests.map(({ method, url }) => [method, url]), [
    ["POST", "/api/v1/auth/login"], ["GET", "/api/v1/snapshot"],
    ["GET", "/api/v1/sources/source-camera/publishing-endpoint"],
    ["POST", "/api/v1/sources/source-camera/recovery/enroll"],
  ])
  assert.deepEqual(JSON.parse(fixture.requests.at(-1).body), { rotate: false })
})

test("--rotate-recovery requires opt-in and sends explicit rotate true", async (context) => {
  const rejected = await setup(context, enrollment())
  const bad = await runCli([...rejected.args, "--rotate-recovery"])
  assert.notEqual(bad.code, 0)
  assert.match(bad.stderr, /requires --with-recovery/u)
  assert.equal(rejected.requests.length, 0)

  const rotated = await setup(context, enrollment({ controllerToken: "rotated-source-token" }))
  const good = await runCli([...rotated.args, "--with-recovery", "--rotate-recovery"])
  assert.equal(good.code, 0, good.stderr)
  assert.deepEqual(JSON.parse(rotated.requests.at(-1).body), { rotate: true })
  const bundle = JSON.parse(await readFile(rotated.output, "utf8"))
  assert.equal(bundle.recovery.controllerToken, "rotated-source-token")
})

for (const [label, controlPath] of [
  ["absolute response URL", "https://evil.example/api/v1/source-recovery/source-camera/poll"],
  ["wrong source path", "/api/v1/source-recovery/source-other/poll"],
  ["generic command path", "/api/v1/commands"],
]) {
  test(`rejects ${label} without writing the one-time controller token`, async (context) => {
    const fixture = await setup(context, enrollment({ controlPath }))
    const result = await runCli([...fixture.args, "--with-recovery"])
    assert.notEqual(result.code, 0)
    assert.match(result.stderr, /controlPath/u)
    assert.equal(await readFile(fixture.output, "utf8").then(() => true, () => false), false)
    assert.equal(`${result.stdout}${result.stderr}`.includes(controllerToken), false)
  })
}

test("ordinary bundle generation never calls enrollment or adds recovery credentials", async (context) => {
  const fixture = await setup(context, enrollment())
  const result = await runCli(fixture.args)
  assert.equal(result.code, 0, result.stderr)
  const bundle = JSON.parse(await readFile(fixture.output, "utf8"))
  assert.equal("recovery" in bundle, false)
  assert.equal(fixture.requests.some(({ url }) => url.includes("/recovery/")), false)
})

async function setup(context, enrolled) {
  const directory = await mkdtemp(join(tmpdir(), "publisher-recovery-"))
  context.after(() => rm(directory, { recursive: true, force: true }))
  const passwordFile = join(directory, "password")
  const output = join(directory, "private", "camera.publisher.json")
  await writeFile(passwordFile, password, { mode: 0o600 })
  const requests = []
  const server = createServer(async (request, response) => {
    const body = await readBody(request)
    requests.push({ method: request.method, url: request.url, cookie: request.headers.cookie, authorization: request.headers.authorization, body })
    response.setHeader("content-type", "application/json")
    if (request.url === "/api/v1/auth/login") {
      response.setHeader("set-cookie", "live-production=test-session; HttpOnly")
      response.end("{}")
    } else if (request.url === "/api/v1/snapshot") {
      response.end(JSON.stringify(snapshot()))
    } else if (request.url === "/api/v1/sources/source-camera/publishing-endpoint") {
      response.end(JSON.stringify(endpoint()))
    } else if (request.url === "/api/v1/sources/source-camera/recovery/enroll") {
      response.setHeader("cache-control", "no-store, private")
      response.end(JSON.stringify(enrolled))
    } else {
      response.statusCode = 404
      response.end("{}")
    }
  })
  await new Promise((resolve, reject) => { server.once("error", reject); server.listen(0, "127.0.0.1", resolve) })
  context.after(() => new Promise((resolve, reject) => server.close((error) => error ? reject(error) : resolve())))
  const address = server.address()
  assert(address !== null && typeof address === "object")
  const origin = `http://127.0.0.1:${address.port}`
  return {
    origin, output, requests,
    args: ["--server-url", `${origin}/api/v1`, "--password-file", passwordFile,
      "--source-id", "source-camera", "--obs-source-name", "Camera", "--audio-mix-index", "0",
      "--target-host", "127.0.0.1", "--output", output],
  }
}

function enrollment(overrides = {}) {
  return {
    schemaVersion: 1, sourceId: "source-camera",
    controllerId: "11111111-1111-4111-8111-111111111111",
    controllerToken, controlPath: "/api/v1/source-recovery/source-camera/poll", policyVersion: "1",
    ...overrides,
  }
}

function snapshot() {
  return { project: { sources: [{ id: "source-camera", adapter: { kind: "srt" }, tracks: [
    { id: "video-camera", kind: "video" }, { id: "audio-camera", kind: "audio" },
  ] }] } }
}

function endpoint() {
  return { sourceId: "source-camera", protocol: "srt", port: 12_345, bufferMs: 120,
    passphrase: "scoped-srt-passphrase", streamId: "#!::r=camera" }
}

async function runCli(args) {
  const child = spawn(process.execPath, [cli, ...args], { stdio: ["ignore", "pipe", "pipe"] })
  const stdout = []; const stderr = []
  child.stdout.on("data", (chunk) => stdout.push(chunk)); child.stderr.on("data", (chunk) => stderr.push(chunk))
  const code = await new Promise((resolve, reject) => { child.once("error", reject); child.once("exit", resolve) })
  return { code, stdout: Buffer.concat(stdout).toString("utf8"), stderr: Buffer.concat(stderr).toString("utf8") }
}

async function readBody(request) {
  const chunks = []
  for await (const chunk of request) chunks.push(chunk)
  return Buffer.concat(chunks).toString("utf8")
}
