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
  recordings: [{
    id: "recording-a", outputId: "output-a", name: "Clean Camera", path: "/recordings/program-001.mkv", tapPoint: "clean_source",
    sessionId: "session-a", recoveryStatus: "recovered", mappingStatus: "unmapped", startNs: "unknown", endNs: null,
    engineRunId: "run-a", mediaStartNs: null, mediaEndNs: null, mediaUncertaintyNs: null,
    bytes: "4096", validation: "partial", sha256: null, gaps: ["recording_failed"],
    provenance: [{ engine_run_id: "run-a", source_id: "source-a", track_id: "track-a", track_kind: "video", continuity_generation: "2", local_start_ns: "10", local_end_ns: "20", media_start_ns: "30", media_end_ns: "40" }],
    provenanceGaps: [{ source_id: "source-a", track_id: "track-a", from_generation: "1", to_generation: "2", local_start_ns: "1", local_end_ns: "2", media_start_ns: "3", media_end_ns: "4" }],
  }],
  plugins: [],
  storage: { freeBytes: "100000", totalBytes: "200000", writable: true },
}

describe("recording library truthfulness", () => {
  it("renders observed recording evidence without inventing a download route", () => {
    const markup = renderToStaticMarkup(<RecordingLibrary snapshot={snapshot} />)

    expect(markup).toContain("4096")
    expect(markup).toContain("RECOVERED")
    expect(markup).toContain("PARTIAL")
    expect(markup).toContain("UNMAPPED")
    expect(markup).toContain("recording_failed")
    expect(markup).toContain("source-a")
    expect(markup).toContain("generation 2")
    expect(markup).toContain("Unknown")
    expect(markup).not.toContain("href=")
    expect(markup).not.toContain("/recordings/program-001.mkv")
  })
})
