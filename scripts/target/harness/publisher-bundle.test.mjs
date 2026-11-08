import assert from "node:assert/strict"
import test from "node:test"
import { createInvalidAuthBundle, createPublisherBundle } from "./publisher-bundle.mjs"

const snapshot = {
  project: {
    sources: [{
      id: "source-red",
      tracks: [
        { id: "video-red", kind: "video" },
        { id: "audio-red", kind: "audio" },
      ],
    }],
  },
}
const endpoint = {
  sourceId: "source-red",
  protocol: "srt",
  port: 12_345,
  passphrase: "scoped-test-passphrase",
  streamId: "#!::u=source-red,r=video-red-audio-red",
  bufferMs: 120,
}
const encoding = {
  width: 1280,
  height: 720,
  fpsNumerator: 30,
  fpsDenominator: 1,
  videoBitrateKbps: 2500,
}

test("creates exact contract-v1 bundle from observed IDs", () => {
  const bundle = createPublisherBundle({
    snapshot,
    endpoint,
    host: "127.0.0.1",
    sourceName: "OBS red",
    audioMixIndex: 0,
    encoding,
  })
  assert.equal(bundle.schemaVersion, 1)
  assert.deepEqual(bundle.target, {
    sourceId: "source-red",
    videoTrackId: "video-red",
    audioTrackId: "audio-red",
    host: "127.0.0.1",
    port: 12_345,
    passphrase: "scoped-test-passphrase",
    streamId: "#!::u=source-red,r=video-red-audio-red",
    bufferMs: 120,
  })
  assert.deepEqual(bundle.obs, { sourceName: "OBS red", audioMixIndex: 0 })
  assert.equal(bundle.encoding.audioSampleRate, 48_000)
})

test("negative bundle changes only publisher authentication", () => {
  const bundle = createPublisherBundle({ snapshot, endpoint, host: "localhost", sourceName: "OBS red", audioMixIndex: 5, encoding })
  const invalid = createInvalidAuthBundle(bundle)
  assert.notEqual(invalid.target.passphrase, bundle.target.passphrase)
  assert.deepEqual({ ...invalid.target, passphrase: bundle.target.passphrase }, bundle.target)
})

test("rejects guessed IDs and invalid OBS mix indexes", () => {
  assert.throws(() => createPublisherBundle({ snapshot, endpoint: { ...endpoint, sourceId: "guessed" }, host: "localhost", sourceName: "bad", audioMixIndex: 0, encoding }))
  assert.throws(() => createPublisherBundle({ snapshot, endpoint, host: "localhost", sourceName: "bad", audioMixIndex: 6, encoding }))
})
