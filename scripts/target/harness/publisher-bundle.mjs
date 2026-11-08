import { join } from "node:path"
import { writePrivateJson } from "./private-files.mjs"

export function createPublisherBundle(input) {
  const { snapshot, endpoint, host, sourceName, sourceUuid, audioMixIndex, encoding } = input
  const source = snapshot.project.sources.find((candidate) => candidate.id === endpoint.sourceId)
  if (source === undefined) throw new Error("Publishing endpoint source is absent from snapshot")
  const tracks = Object.fromEntries(source.tracks.map((track) => [track.kind, track.id]))
  if (tracks.video === undefined || tracks.audio === undefined) {
    throw new Error("Source must expose observed video and audio track IDs")
  }
  if (endpoint.protocol !== "srt" || typeof endpoint.passphrase !== "string" || endpoint.passphrase.length < 10) {
    throw new Error("Publishing endpoint is not an authenticated SRT contract")
  }
  if (typeof endpoint.streamId !== "string" || endpoint.streamId.length === 0) {
    throw new Error("Publishing endpoint omitted its scoped streamId")
  }
  if (!Number.isInteger(audioMixIndex) || audioMixIndex < 0 || audioMixIndex > 5) {
    throw new Error("OBS audioMixIndex must be an integer from 0 through 5")
  }
  validateEncoding(encoding)
  const obs = { sourceName, audioMixIndex }
  if (typeof sourceUuid === "string" && sourceUuid.length > 0) obs.sourceUuid = sourceUuid
  return {
    schemaVersion: 1,
    target: {
      sourceId: source.id,
      videoTrackId: tracks.video,
      audioTrackId: tracks.audio,
      host,
      port: endpoint.port,
      passphrase: endpoint.passphrase,
      streamId: endpoint.streamId,
      bufferMs: endpoint.bufferMs,
    },
    obs,
    encoding: { ...encoding, audioSampleRate: 48_000 },
  }
}

export async function writePublisherBundle(directory, label, input) {
  const bundle = createPublisherBundle(input)
  const path = join(directory, `${label}.publisher.json`)
  await writePrivateJson(path, bundle)
  return { path, bundle }
}

export function createInvalidAuthBundle(bundle) {
  return {
    ...bundle,
    target: { ...bundle.target, passphrase: `invalid-${bundle.target.passphrase}` },
  }
}

function validateEncoding(encoding) {
  for (const key of ["width", "height", "fpsNumerator", "fpsDenominator", "videoBitrateKbps"]) {
    if (!Number.isInteger(encoding[key]) || encoding[key] <= 0) {
      throw new Error(`Encoding ${key} must be a positive integer`)
    }
  }
}
