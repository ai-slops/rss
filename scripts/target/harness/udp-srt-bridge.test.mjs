import assert from "node:assert/strict"
import dgram from "node:dgram"
import test from "node:test"
import { UdpSrtBridge } from "./udp-srt-bridge.mjs"

test("an unused authorized process-resume handover can be revoked without opening another peer", async (context) => {
  const server = await socket()
  context.after(() => close(server))
  const bridge = new UdpSrtBridge({ serverPort: server.address().port })
  await bridge.start()
  context.after(() => bridge.close())
  const authorized = bridge.authorizeHandover()
  bridge.cancelHandover()
  assert.equal(await authorized, null)
  const next = bridge.authorizeHandover()
  bridge.cancelHandover()
  assert.equal(await next, null)
  assert.equal(bridge.stats().handovers, 0)
})

test("requires an explicit control-packet handover and preserves old control routing", async (context) => {
  const server = await socket(); const first = await socket(); const second = await socket(); const unknown = await socket()
  for (const value of [server, first, second, unknown]) context.after(() => close(value))
  const serverAddress = server.address(); assert(typeof serverAddress === "object")
  const bridge = new UdpSrtBridge({ serverPort: serverAddress.port })
  const frontPort = await bridge.start(); context.after(() => bridge.close())

  first.send(data(1), frontPort, "127.0.0.1")
  first.send(control(2), frontPort, "127.0.0.1")
  const initial = await packet(server); assert.deepEqual(initial.message, control(2))
  const firstServerPort = initial.remote.port
  server.send(control(3), firstServerPort, "127.0.0.1")
  assert.deepEqual((await packet(first)).message, control(3))

  unknown.send(control(4), frontPort, "127.0.0.1")
  first.send(control(40), frontPort, "127.0.0.1")
  assert.deepEqual((await packet(server)).message, control(40))
  const handover = bridge.authorizeHandover()
  second.send(data(5), frontPort, "127.0.0.1")
  second.send(control(6), frontPort, "127.0.0.1")
  const switched = await packet(server); assert.deepEqual(switched.message, control(6))
  const transition = await handover
  assert.notEqual(transition.oldPeerPort, transition.newPeerPort)
  assert.notEqual(transition.oldServerPort, transition.newServerPort)
  assert.equal(switched.remote.port, transition.newServerPort)

  first.send(data(7), frontPort, "127.0.0.1")
  first.send(control(8), frontPort, "127.0.0.1")
  assert.deepEqual((await packet(server)).message, control(8))
  server.send(control(9), transition.oldServerPort, "127.0.0.1")
  assert.deepEqual((await packet(first)).message, control(9))
  server.send(data(10), transition.newServerPort, "127.0.0.1")
  assert.deepEqual((await packet(second)).message, data(10))

  await bridge.retirePreviousPeer()
  bridge.pauseData()
  second.send(data(11), frontPort, "127.0.0.1")
  second.send(control(12), frontPort, "127.0.0.1")
  assert.deepEqual((await packet(server)).message, control(12))
  bridge.resumeData()
  assert.deepEqual(bridge.stats(), {
    forwardedData: 0, forwardedControl: 5, droppedData: 2, rejectedPeers: 3, handovers: 1,
    dataPaused: false, activePeerPort: transition.newPeerPort,
    activeServerPort: transition.newServerPort, previousPeerPort: null, previousServerPort: null,
  })
})

function data(value) { return Buffer.from([0x00, value]) }
function control(value) { return Buffer.from([0x80, value]) }
function socket() { const value = dgram.createSocket("udp4"); return new Promise((resolve, reject) => { value.once("error", reject); value.bind(0, "127.0.0.1", () => resolve(value)) }) }
function packet(socketValue) { return new Promise((resolve, reject) => { const timer = setTimeout(() => reject(new Error("UDP message timeout")), 1_000); socketValue.once("message", (message, remote) => { clearTimeout(timer); resolve({ message, remote }) }) }) }
function close(socketValue) { return new Promise((resolve) => socketValue.close(resolve)) }
