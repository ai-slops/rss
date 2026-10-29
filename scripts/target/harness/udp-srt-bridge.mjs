import dgram from "node:dgram"

export class UdpSrtBridge {
  #front = dgram.createSocket("udp4")
  #server
  #active = null
  #previous = null
  #handover = null
  #paused = false
  #closed = false
  #stats = { forwardedData: 0, forwardedControl: 0, droppedData: 0,
    rejectedPeers: 0, handovers: 0 }

  constructor({ serverPort, serverHost = "127.0.0.1" }) {
    if (serverHost !== "127.0.0.1" || !Number.isInteger(serverPort) || serverPort < 1 || serverPort > 65_535) {
      throw new Error("SRT bridge requires one loopback UDP server endpoint")
    }
    this.#server = { address: serverHost, port: serverPort }
  }

  async start() {
    this.#front.on("message", (message, remote) => { void this.#fromPeer(message, remote) })
    await bind(this.#front)
    return this.#front.address().port
  }

  pauseData() { this.#paused = true }
  resumeData() { this.#paused = false }

  authorizeHandover(timeoutMs = 15_000) {
    if (this.#handover !== null || this.#previous !== null) throw new Error("SRT bridge already has a handover in progress")
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.#handover = null
        reject(new Error("SRT bridge authorized handover timed out"))
      }, timeoutMs)
      this.#handover = { resolve: (value) => { clearTimeout(timer); this.#handover = null; resolve(value) }, reject }
    })
  }

  async retirePreviousPeer() {
    if (this.#previous === null) return
    await closeSocket(this.#previous.back)
    this.#previous = null
  }

  stats() {
    return { ...this.#stats, dataPaused: this.#paused,
      activePeerPort: this.#active?.peer.port ?? null,
      activeServerPort: socketPort(this.#active?.back),
      previousPeerPort: this.#previous?.peer.port ?? null,
      previousServerPort: socketPort(this.#previous?.back) }
  }

  async close() {
    if (this.#closed) return
    this.#closed = true
    if (this.#handover !== null) this.#handover.reject(new Error("SRT bridge closed during handover"))
    await Promise.all([this.#active?.back, this.#previous?.back, this.#front]
      .filter(Boolean).map((socket) => closeSocket(socket)))
    this.#active = null; this.#previous = null
  }

  async #fromPeer(message, remote) {
    if (this.#closed || remote.address !== "127.0.0.1" || message.length === 0) return
    const control = (message[0] & 0x80) !== 0
    let route = this.#route(remote)
    if (route === null) {
      if (!control || (this.#active !== null && this.#handover === null)) {
        this.#stats.rejectedPeers += 1
        return
      }
      route = await this.#newRoute(remote)
      if (this.#active !== null) {
        this.#previous = this.#active
        this.#active = route
        this.#stats.handovers += 1
        this.#handover.resolve({ oldPeerPort: this.#previous.peer.port, newPeerPort: route.peer.port,
          oldServerPort: socketPort(this.#previous.back), newServerPort: socketPort(route.back) })
      } else {
        this.#active = route
      }
    }
    const previousRoute = route === this.#previous
    if (!control && (this.#paused || previousRoute)) { this.#stats.droppedData += 1; return }
    if (control) this.#stats.forwardedControl += 1
    else this.#stats.forwardedData += 1
    route.back.send(message, this.#server.port, this.#server.address)
  }

  async #newRoute(remote) {
    const back = dgram.createSocket("udp4")
    const route = { peer: { address: remote.address, port: remote.port }, back }
    back.on("message", (message) => {
      if (!this.#closed) this.#front.send(message, route.peer.port, route.peer.address)
    })
    await bind(back)
    return route
  }

  #route(remote) {
    for (const route of [this.#active, this.#previous]) {
      if (route?.peer.port === remote.port && route.peer.address === remote.address) return route
    }
    return null
  }
}

function bind(socket) {
  return new Promise((resolve, reject) => {
    socket.once("error", reject)
    socket.bind(0, "127.0.0.1", resolve)
  })
}
function closeSocket(socket) { return new Promise((resolve) => socket.close(resolve)) }
function socketPort(socket) { if (socket === undefined || socket === null) return null; const value = socket.address(); return typeof value === "object" ? value.port : null }
