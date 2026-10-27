import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, it } from "vitest"
import type { Snapshot } from "../api/schema"
import { RecordingLibrary } from "./RecordingLibrary"

const snapshot: Snapshot = {
  cursor: "1",
  serverTime: "now",
  engineRunId: "engine-a",
  project: {
    id: "project-a",
    name: "Production",
    revision: "1",
    canvas: { width: 960, height: 540, fpsNumerator: 30, fpsDenominator: 1 },
    scenes: [{ id: "scene-a", name: "Standby", revision: "1", items: [] }],
    sources: [],
    audioRoutes: [],
    holdbackMs: 500,
    outputs: [],
    assets: [],
  },
  preview: { sceneId: "scene-a", scene: { id: "scene-a", name: "Standby", revision: "1", items: [] }, revision: "1", renderedRevision: null, visibleRevision: null, mediaState: "disconnected" },
  program: { sceneId: "scene-a", appliedScene: { id: "scene-a", name: "Standby", revision: "1", items: [] }, sceneRevision: "1", programRevision: "1", appliedAtNs: "0", health: "healthy", previousSceneId: null },
  outputStatuses: [],
  recordings: [{ id: "recording-a", outputId: "output-a", name: "Program 001", path: "/recordings/program-001.mkv", tapPoint: "program", startNs: "10", endNs: "20", bytes: "4096", validation: "partial", sha256: null, gaps: ["source gap"] }],
  plugins: [],
  storage: { freeBytes: "100000", totalBytes: "200000", writable: true },
}

describe("recording library truthfulness", () => {
  it("renders observed recording evidence without inventing a download route", () => {
    const markup = renderToStaticMarkup(<RecordingLibrary snapshot={snapshot} />)

    expect(markup).toContain("4096")
    expect(markup).toContain("source gap")
    expect(markup).not.toContain("href=")
  })
})
