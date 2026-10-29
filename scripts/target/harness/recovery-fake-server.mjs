import { createServer } from "node:http"
import { RecoveryModel } from "./recovery-model.mjs"

export async function startRecoveryFakeServer(options) {
  let model = new RecoveryModel(options)
  const requests = []
  const server = createServer(async (request, response) => {
    const body = await readJson(request, 32_768)
    requests.push({ method: request.method, url: request.url,
      cookiePresent: request.headers.cookie !== undefined,
      operatorHeaderPresent: request.headers["x-live-production"] !== undefined })
    response.setHeader("content-type", "application/json")
    const match = request.url?.match(/^\/api\/v1\/source-recovery\/([^/]+)\/poll$/u)
    if (request.method !== "POST" || match === null) {
      response.statusCode = 404; response.end("{}"); return
    }
    const authorization = request.headers.authorization
    const token = authorization?.startsWith("Bearer ") === true ? authorization.slice(7) : null
    let result
    try {
      result = model.poll({ sourceId: decodeURIComponent(match[1]), token, request: body, nowMs: Date.now() })
    } catch {
      result = { status: 400, body: null }
    }
    response.statusCode = result.status
    response.end(result.body === null ? "{}" : JSON.stringify(result.body))
  })
  await new Promise((resolve, reject) => {
    server.once("error", reject)
    server.listen(0, "127.0.0.1", resolve)
  })
  const address = server.address()
  if (address === null || typeof address !== "object") throw new Error("Recovery fake server did not bind TCP")
  return {
    baseUrl: `http://127.0.0.1:${address.port}/api/v1`,
    requests,
    model: () => model,
    restart: (serverRunId) => { model = new RecoveryModel({ ...options, serverRunId, persisted: model.persisted() }) },
    close: () => new Promise((resolve, reject) => server.close((error) => error ? reject(error) : resolve())),
  }
}

async function readJson(request, maximumBytes) {
  const chunks = []; let length = 0
  for await (const chunk of request) {
    length += chunk.length
    if (length > maximumBytes) throw new Error("Request body exceeds recovery fake limit")
    chunks.push(chunk)
  }
  return JSON.parse(Buffer.concat(chunks).toString("utf8"))
}
