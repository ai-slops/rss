import { readJson } from "./private-files.mjs"
import { poll } from "./processes.mjs"

export async function waitPluginReport(path, predicate, timeoutMs) {
  let report
  await poll(async () => {
    report = await readJson(path).catch(() => null)
    return report !== null && predicate(report)
  }, timeoutMs, "OBS recovery fixture report")
  return report
}

export async function waitRecoveryStatus(api, sourceId, predicate, timeoutMs) {
  let status
  await poll(async () => { status = await api.recoveryStatus(sourceId); return predicate(status) },
    timeoutMs, "source recovery status")
  return status
}

export function usableReceiver(receiver) {
  return receiver.state === "healthy" && numeric(receiver.videoFrames) > 20
    && numeric(receiver.audioBuffers) > 20 && numeric(receiver.lastVideoPtsNs) > 0
}

export function numeric(value) { return Number.parseInt(value ?? "0", 10) }

export function rawControllerPoll(origin, sourceId, token, controllerId) {
  return fetch(`${origin}/api/v1/source-recovery/${sourceId}/poll`, { method: "POST",
    headers: { "content-type": "application/json", ...(token === null ? {} : { authorization: `Bearer ${token}` }) },
    body: JSON.stringify({ schemaVersion: 1, controllerId, processEpoch: "1", pollSeq: "1", intentEpoch: "1",
      localDesired: "stopped", publisherState: "stopped", lastAction: null }), redirect: "error" })
}
