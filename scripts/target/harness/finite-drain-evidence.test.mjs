import assert from "node:assert/strict"
import test from "node:test"
import { finiteTransferComplete } from "./finite-drain-evidence.mjs"

const source = { strictDecoded: true, frames: [
  { kind: "video", codec: "h264", frames: "150" }, { kind: "audio", codec: "aac", frames: "235" },
] }
test("complete finite sender and receiver A/V counts pass", () => {
  assert.equal(finiteTransferComplete(source, structuredClone(source)), true)
})
test("strict decode alone cannot hide whole AAC or video loss", () => {
  const lostAudio = structuredClone(source); lostAudio.frames[1].frames = "229"
  const lostVideo = structuredClone(source); lostVideo.frames[0].frames = "147"
  assert.equal(finiteTransferComplete(source, lostAudio), false)
  assert.equal(finiteTransferComplete(source, lostVideo), false)
})
test("missing audio and unknown frame counts fail closed", () => {
  assert.equal(finiteTransferComplete(source, { strictDecoded: true, frames: source.frames.slice(0, 1) }), false)
  const unknown = structuredClone(source); unknown.frames[1].frames = "N/A"
  assert.equal(finiteTransferComplete(unknown, unknown), false)
})
