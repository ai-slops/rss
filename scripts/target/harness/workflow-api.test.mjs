import assert from "node:assert/strict"
import test from "node:test"
import { configureProduction } from "./workflow-api.mjs"

test("uses registered scene, audio, timing and Program Take contracts", async () => {
  const api = new FakeApi()
  const sources = api.state.project.sources.map((source, index) => ({ key: index === 0 ? "red" : "blue", source }))
  const result = await configureProduction(api, sources)
  assert.deepEqual(api.calls.map((call) => call.kind), [
    "scene.item.add", "scene.item.add", "scene.save", "audio.apply", "timing.apply", "program.take",
  ])
  const take = api.calls.at(-1)
  assert.deepEqual(take.payload, { sceneId: "scene-a", revision: "4" })
  assert.equal(take.options.expectedRevision, "program-1")
  assert.equal(result.snapshot.program.sceneId, "scene-a")
  assert.equal(result.snapshot.project.audioRoutes.length, 2)
})

class FakeApi {
  calls = []
  state = fixture()

  async snapshot() { return structuredClone(this.state) }

  async command(kind, targetId, payload, options) {
    this.calls.push({ kind, targetId, payload, options })
    const scene = this.state.project.scenes[0]
    if (kind === "scene.item.add") {
      const source = this.state.project.sources.find((candidate) => candidate.id === payload.sourceId)
      scene.items.push(item(source, scene.items.length)); scene.revision = `${Number(scene.revision) + 1}`
    } else if (kind === "scene.save") {
      this.state.project.scenes[0] = { ...structuredClone(payload.scene), revision: `${Number(payload.scene.revision) + 1}` }
    } else if (kind === "audio.apply") {
      this.state.project.audioRoutes = structuredClone(payload.routes); this.state.project.revision = "2"
    } else if (kind === "timing.apply") {
      this.state.project.holdbackMs = payload.holdbackMs; this.state.project.revision = "3"
    } else if (kind === "program.take") {
      this.state.program.sceneId = payload.sceneId; this.state.program.programRevision = "program-2"
    }
    return { status: "applied", actualRevision: scene.revision }
  }
}

function fixture() {
  return {
    engineRunId: "engine-a",
    project: {
      id: "project-a", revision: "1", canvas: { width: 960, height: 540 }, audioRoutes: [], holdbackMs: 500,
      scenes: [{ id: "scene-a", name: "Standby", revision: "1", items: [] }],
      sources: [source("red"), source("blue")], outputs: [],
    },
    preview: { sceneId: "scene-a", revision: "1" },
    program: { sceneId: "scene-a", programRevision: "program-1" },
  }
}

function source(id) {
  return { id, name: id, tracks: [{ id: `${id}-video`, kind: "video" }, { id: `${id}-audio`, kind: "audio" }] }
}

function item(sourceValue, index) {
  return {
    id: `item-${index}`, name: sourceValue.name, visible: true, locked: false,
    rect: { x: 0, y: 0, width: 960, height: 540 }, crop: { left: 0, top: 0, right: 0, bottom: 0 },
    fit: "contain", opacity: 1, content: { kind: "track", trackId: `${sourceValue.id}-video` },
  }
}
