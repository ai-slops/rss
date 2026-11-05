import { randomBytes } from "node:crypto"
import assert from "node:assert/strict"
import { join } from "node:path"
import { createPublisherBundle } from "./publisher-bundle.mjs"
import { writePrivateJson } from "./private-files.mjs"
import { freeUdpPort } from "./processes.mjs"
import { startRecoveryObs, startServer, startVirtualDisplay } from "./native-runtime.mjs"
import { writeRecoveryFaultFixture } from "./recovery-faults.mjs"
import { updateRecoveryPolicy } from "./recovery-evidence.mjs"
import { UdpSrtBridge } from "./udp-srt-bridge.mjs"

export async function startRecoveryFixture(options, privateDirectory, processes) {
  const password = randomBytes(32).toString("hex")
  const server = await startServer(processes, options, privateDirectory, password, "recovery-initial")
  const initial = await server.api.snapshot(); const port = await freeUdpPort()
  const bridge = new UdpSrtBridge({ serverPort: port })
  try {
    const bridgePort = await bridge.start()
    await server.api.command("source.create", initial.project.id, { name: "Recovery OBS Red 440",
      adapter: { kind: "srt", bindAddress: "127.0.0.1", port, bufferMs: 120 } }, { snapshot: initial })
    const snapshot = await server.api.snapshot()
    const source = snapshot.project.sources.find(candidate => candidate.name === "Recovery OBS Red 440")
    if (source === undefined) throw new Error("Recovery source creation did not persist")
    const baseline = { outputs: snapshot.project.outputs.length, programRevision: snapshot.program.programRevision }
    const endpoint = await server.api.publishingEndpoint(source.id)
    const policy = await updateRecoveryPolicy(server.api, source.id, { enabled: true, maintenance: false })
    const enrollment = await server.api.enrollRecovery(source.id, false)
    assert.equal(enrollment.policyVersion, policy.after.policyVersion)
    const bundle = createPublisherBundle({ snapshot, endpoint: { ...endpoint, port: bridgePort }, host: "127.0.0.1",
      sourceName: "Recovery OBS Red 440", audioMixIndex: 0,
      encoding: { width: 1280, height: 720, fpsNumerator: 30, fpsDenominator: 1, videoBitrateKbps: 2500 } })
    bundle.recovery = { schemaVersion: 1, controlUrl: `${server.origin}${enrollment.controlPath}`,
      controllerId: enrollment.controllerId, controllerToken: enrollment.controllerToken }
    const bundlePath = join(privateDirectory, "recovery.publisher.json")
    await writePrivateJson(bundlePath, bundle)
    const faults = await writeRecoveryFaultFixture(privateDirectory,
      { sourceId: source.id, publisherBundle: bundlePath, deadlineSeconds: options.deadlineSeconds })
    const display = await startVirtualDisplay(processes, options, privateDirectory)
    const obs = await startRecoveryObs(processes, options, privateDirectory, faults.fixtureFile, display.display)
    return { server, obs, bridge, password, source, baseline, policy, enrollment, faults, display: display.display }
  } catch (error) {
    await bridge.close()
    throw error
  }
}
