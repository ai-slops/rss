import assert from "node:assert/strict"
import { execFile } from "node:child_process"
import { createServer } from "node:http"
import { mkdtemp, rm, symlink, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { promisify } from "node:util"
import test from "node:test"

const execute = promisify(execFile)
const cli = new URL("./publisher-bundle.mjs", import.meta.url).pathname
const password = "private-redirect-regression-password"

test("rejects login redirects without forwarding the operator credential", async (context) => {
  const directory = await mkdtemp(join(tmpdir(), "publisher-redirect-"))
  context.after(() => rm(directory, { recursive: true, force: true }))
  const passwordFile = join(directory, "operator-password")
  await writeFile(passwordFile, password, { mode: 0o600 })
  let forwardedRequests = 0
  let loginRequests = 0
  const destination = createServer((request, response) => {
    forwardedRequests += 1
    request.resume()
    response.end("{}")
  })
  const destinationPort = await listen(destination)
  context.after(() => close(destination))
  const control = createServer((request, response) => {
    loginRequests += 1
    request.resume()
    response.writeHead(307, { location: `http://127.0.0.1:${destinationPort}/receive` })
    response.end()
  })
  const controlPort = await listen(control)
  context.after(() => close(control))

  const result = await execute(process.execPath, argumentsFor(directory, passwordFile, controlPort))
    .then(() => ({ failed: false, stderr: "" }), error => ({ failed: true, stderr: error.stderr }))

  assert.equal(result.failed, true)
  assert.equal(loginRequests, 1)
  assert.equal(forwardedRequests, 0)
  assert.equal(result.stderr.includes(password), false)
})

test("rejects symlinked and oversized password files before authentication", async (context) => {
  const directory = await mkdtemp(join(tmpdir(), "publisher-private-file-"))
  context.after(() => rm(directory, { recursive: true, force: true }))
  const original = join(directory, "original")
  const link = join(directory, "link")
  const oversized = join(directory, "oversized")
  await writeFile(original, password, { mode: 0o600 })
  await symlink(original, link)
  await writeFile(oversized, "x".repeat(4097), { mode: 0o600 })

  for (const path of [link, oversized]) {
    const result = await execute(process.execPath, argumentsFor(directory, path, 9))
      .then(() => ({ failed: false, stderr: "" }), error => ({ failed: true, stderr: error.stderr }))
    assert.equal(result.failed, true)
    assert.equal(result.stderr.includes("fetch failed"), false)
    assert.equal(result.stderr.includes(password), false)
  }
})

function argumentsFor(directory, passwordFile, port) {
  return [cli, "--server-url", `http://127.0.0.1:${port}/api/v1`,
    "--password-file", passwordFile, "--source-id", "source-camera",
    "--obs-source-name", "Camera", "--audio-mix-index", "0",
    "--output", join(directory, "bundle.json")]
}

async function listen(server) {
  await new Promise((resolve, reject) => {
    server.once("error", reject)
    server.listen(0, "127.0.0.1", resolve)
  })
  const address = server.address()
  assert(address !== null && typeof address === "object")
  return address.port
}

function close(server) {
  return new Promise((resolve, reject) => server.close(error => error ? reject(error) : resolve()))
}
