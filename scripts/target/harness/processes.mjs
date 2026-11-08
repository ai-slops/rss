import { open, readFile, stat } from "node:fs/promises"
import net from "node:net"
import dgram from "node:dgram"
import { spawn } from "node:child_process"

export class OwnedProcesses {
  #children = []
  #supervisorBinary
  #timeoutMs
  #graceMs

  constructor(supervisorBinary, options) {
    this.#supervisorBinary = supervisorBinary
    this.#timeoutMs = options.timeoutMs
    this.#graceMs = options.graceMs
  }

  async spawn(label, command, args, options = {}) {
    const log = await open(options.logPath, "a", 0o600)
    const supervisor = spawn(this.#supervisorBinary, [
      "--timeout-ms", `${this.#timeoutMs}`, "--grace-ms", `${this.#graceMs}`, "--",
      command, ...args,
    ], {
      cwd: options.cwd,
      env: options.env,
      stdio: ["ignore", log.fd, log.fd],
    })
    await log.close()
    const record = { label, supervisor, nativePid: null, stopped: false }
    this.#children.push(record)
    supervisor.once("exit", () => { record.stopped = true })
    supervisor.once("error", () => { record.stopped = true })
    record.nativePid = await directChildPid(supervisor.pid, 10_000)
    return record
  }

  signalNative(record, signal) {
    if (record.nativePid === null) throw new Error(`${record.label} has no supervised native PID`)
    process.kill(record.nativePid, signal)
  }

  async wait(record, timeoutMs = 10_000) {
    await waitForExit(record.supervisor, timeoutMs)
    record.stopped = true
  }

  async stop(record, timeoutMs = 10_000) {
    if (record.stopped) return
    if (record.nativePid !== null) {
      try { process.kill(record.nativePid, "SIGCONT") } catch (error) { if (error.code !== "ESRCH") throw error }
    }
    record.supervisor.kill("SIGTERM")
    await this.wait(record, timeoutMs).catch(async () => {
      record.supervisor.kill("SIGKILL")
      await this.wait(record, 2_000).catch(() => undefined)
    })
  }

  async stopAll() {
    for (const record of [...this.#children].reverse()) await this.stop(record)
  }
}

export async function freeTcpPort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer()
    server.once("error", reject)
    server.listen(0, "127.0.0.1", () => {
      const address = server.address()
      const port = typeof address === "object" && address !== null ? address.port : null
      server.close(() => port === null ? reject(new Error("No ephemeral port")) : resolve(port))
    })
  })
}

export async function freeUdpPort() {
  return new Promise((resolve, reject) => {
    const socket = dgram.createSocket("udp4")
    socket.once("error", reject)
    socket.bind(0, "127.0.0.1", () => {
      const port = socket.address().port
      socket.close(() => resolve(port))
    })
  })
}

export async function waitForFile(path, timeoutMs, record = null) {
  await poll(async () => {
    if (await stat(path).then(() => true).catch(() => false)) return true
    if (record !== null && (record.supervisor.exitCode !== null || record.supervisor.signalCode !== null)) {
      const cause = { label: record.label, exitCode: record.supervisor.exitCode, signal: record.supervisor.signalCode }
      throw new Error(`${record.label} exited before its report: exit=${cause.exitCode} signal=${cause.signal}`, { cause })
    }
    return false
  }, timeoutMs, `file ${path}`)
}

export async function poll(check, timeoutMs, label) {
  const deadline = Date.now() + timeoutMs
  while (Date.now() < deadline) {
    if (await check()) return
    await new Promise((resolve) => setTimeout(resolve, 100))
  }
  throw new Error(`Timed out waiting for ${label}`)
}

async function directChildPid(supervisorPid, timeoutMs) {
  let observed = null
  await poll(async () => {
    const value = await readFile(`/proc/${supervisorPid}/task/${supervisorPid}/children`, "utf8").catch(() => "")
    const child = value.trim().split(/\s+/u).find(Boolean)
    if (child === undefined) return false
    observed = Number.parseInt(child, 10)
    return Number.isInteger(observed) && observed > 1
  }, timeoutMs, `supervisor ${supervisorPid} child`)
  return observed
}

function waitForExit(child, timeoutMs) {
  return new Promise((resolve, reject) => {
    if (child.exitCode !== null || child.signalCode !== null) return resolve()
    const timer = setTimeout(() => reject(new Error("supervisor exit timeout")), timeoutMs)
    child.once("exit", () => { clearTimeout(timer); resolve() })
  })
}
