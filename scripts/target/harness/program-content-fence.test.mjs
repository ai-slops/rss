import assert from "node:assert/strict"
import test from "node:test"
import { assertProgramPreserved } from "./program-content-fence.mjs"

function snapshot(engine = "engine-a", revision = "engine-a.1") {
  return { engineRunId: engine, program: { programRevision: revision, sceneId: "scene-a", sceneRevision: "7",
    appliedScene: { id: "scene-a", revision: "7", items: [{ id: "item-a", x: 0, content: { kind: "text", text: "on air" } }] } },
    project: { id: "project-a", canvas: { width: 960, height: 540 }, assets: [],
      audioRoutes: [{ trackId: "audio-a", gainDb: 0, muted: false, routed: true, inputPeakDb: -6 }] } }
}

test("a daemon restart may reissue Program revision while preserving actual content", () => {
  assert.doesNotThrow(() => assertProgramPreserved(snapshot(), snapshot("engine-b", "engine-b.1")))
})
test("layout and live mix changes cannot hide behind unchanged revision text", () => {
  const moved = snapshot(); moved.program.appliedScene.items[0].x = 200
  assert.throws(() => assertProgramPreserved(snapshot(), moved))
  const muted = snapshot(); muted.project.audioRoutes[0].muted = true
  assert.throws(() => assertProgramPreserved(snapshot(), muted))
})
test("same-engine unexpected Take is still fenced even for identical pixels", () => {
  assert.throws(() => assertProgramPreserved(snapshot(), snapshot("engine-a", "engine-a.2")))
})
test("meter observations are not Program content but scene identity and assets are", () => {
  const metered = snapshot(); metered.project.audioRoutes[0].inputPeakDb = -30
  assert.doesNotThrow(() => assertProgramPreserved(snapshot(), metered))
  const swapped = snapshot(); swapped.program.sceneId = "scene-b"
  assert.throws(() => assertProgramPreserved(snapshot(), swapped))
  const asset = snapshot(); asset.project.assets.push({ id: "asset-a", sha256: "changed" })
  assert.throws(() => assertProgramPreserved(snapshot(), asset))
})
