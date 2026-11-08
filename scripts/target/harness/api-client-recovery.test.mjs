import assert from "node:assert/strict"
import { createServer } from "node:http"
import test from "node:test"
import { ApiClient } from "./api-client.mjs"

test("controller poll uses only the source bearer credential", async (context) => {
  const requests = []
  const server = createServer(async (request, response) => {
    requests.push({ url: request.url, cookie: request.headers.cookie,
      authorization: request.headers.authorization, mutation: request.headers["x-live-production"], body: await body(request) })
    response.setHeader("content-type", "application/json")
    if (request.url === "/api/v1/auth/login") {
      response.setHeader("set-cookie", "live-production=operator-session; HttpOnly")
      response.end("{}")
    } else {
      response.end(JSON.stringify({ schemaVersion: 1, action: null }))
    }
  })
  await new Promise((resolve, reject) => { server.once("error", reject); server.listen(0, "127.0.0.1", resolve) })
  context.after(() => new Promise((resolve, reject) => server.close((error) => error ? reject(error) : resolve())))
  const address = server.address()
  assert(address !== null && typeof address === "object")
  const api = new ApiClient(`http://127.0.0.1:${address.port}/api/v1`)
  await api.login("operator-password")
  await api.pollRecovery("source-camera", "source-controller-token", {
    schemaVersion: 1, controllerId: "controller", processEpoch: "1", pollSeq: "1",
    intentEpoch: "1", localDesired: "stopped", publisherState: "stopped", lastAction: null,
  })
  assert.equal(requests[1].url, "/api/v1/source-recovery/source-camera/poll")
  assert.equal(requests[1].authorization, "Bearer source-controller-token")
  assert.equal(requests[1].cookie, undefined)
  assert.equal(requests[1].mutation, undefined)
})

async function body(request) {
  const chunks = []
  for await (const chunk of request) chunks.push(chunk)
  return Buffer.concat(chunks).toString("utf8")
}
