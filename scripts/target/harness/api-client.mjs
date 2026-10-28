import { randomUUID } from "node:crypto"

export class ApiClient {
  #baseUrl
  #cookie = null

  constructor(baseUrl) {
    this.#baseUrl = baseUrl.replace(/\/$/u, "")
  }

  async login(password) {
    const response = await this.#request("/auth/login", {
      method: "POST",
      body: { password },
      authenticated: false,
    })
    const setCookie = response.headers.get("set-cookie")
    if (setCookie === null) throw new Error("Login response omitted its session cookie")
    this.#cookie = setCookie.split(";", 1)[0]
  }

  async snapshot() {
    return this.#json("/snapshot")
  }

  async publishingEndpoint(sourceId) {
    return this.#json(`/sources/${encodeURIComponent(sourceId)}/publishing-endpoint`)
  }

  async command(kind, targetId, payload, options = {}) {
    const snapshot = options.snapshot ?? await this.snapshot()
    const body = {
      id: options.id ?? `${kind}-${randomUUID()}`,
      kind,
      targetId,
      expectedEngineRunId: snapshot.engineRunId,
      payload,
    }
    if (options.expectedRevision !== undefined) body.expectedRevision = options.expectedRevision
    const receipt = await this.#json("/commands", { method: "POST", body })
    if (receipt.status !== "applied") {
      throw new Error(`${kind} was ${receipt.status}: ${receipt.reason ?? "no reason"}`)
    }
    return receipt
  }

  async #json(path, options = {}) {
    const response = await this.#request(path, options)
    return response.json()
  }

  async #request(path, options = {}) {
    const headers = new Headers({ accept: "application/json" })
    if (options.body !== undefined) headers.set("content-type", "application/json")
    if (options.method !== undefined && options.method !== "GET") headers.set("x-live-production", "1")
    if (options.authenticated !== false) {
      if (this.#cookie === null) throw new Error("Authenticated API request attempted before login")
      headers.set("cookie", this.#cookie)
    }
    const response = await fetch(`${this.#baseUrl}${path}`, {
      method: options.method ?? "GET",
      headers,
      body: options.body === undefined ? undefined : JSON.stringify(options.body),
      signal: AbortSignal.timeout(options.timeoutMs ?? 10_000),
    })
    if (!response.ok) throw new Error(`API ${path} failed with HTTP ${response.status}`)
    return response
  }
}
