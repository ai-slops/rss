import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, it } from "vitest"
import type { Snapshot } from "../api/schema"
import { SyncPanel } from "./SyncPanel"

function snapshotWithDrift(drift: Snapshot["project"]["sources"][number]["tracks"][number]["drift"]): Snapshot {
  return {
    cursor: "1", serverTime: "now", engineRunId: "engine-a",
    project: {
      id: "project-a", name: "Production", revision: "1", canvas: { width: 960, height: 540, fpsNumerator: 30, fpsDenominator: 1 },
      scenes: [{ id: "scene-a", name: "Standby", revision: "1", items: [] }],
      sources: [{ id: "source-a", name: "Camera", enabled: true, adapter: { kind: "synthetic", pattern: "smpte", frequencyHz: 440 }, tracks: [{ id: "video-a", kind: "video", correctionMs: 0, connection: "connected", format: "video", bufferMs: 120, lateFrames: 0, drift }], recordOnConnect: false, dependencyCount: 0 }],
      audioRoutes: [], holdbackMs: 500, outputs: [], assets: [],
    },
    preview: { sceneId: "scene-a", scene: { id: "scene-a", name: "Standby", revision: "1", items: [] }, revision: "1", renderedRevision: null, visibleRevision: null, mediaState: "live" },
    program: { sceneId: "scene-a", appliedScene: { id: "scene-a", name: "Standby", revision: "1", items: [] }, sceneRevision: "1", programRevision: "1", appliedAtNs: "0", health: "healthy", previousSceneId: null },
    outputStatuses: [], recordings: [], plugins: [], storage: { freeBytes: "1", totalBytes: "2", writable: true },
  }
}

describe("SyncPanel drift evidence", () => {
  it("renders Unknown before a measurement window exists", () => {
    const markup = renderToStaticMarkup(<SyncPanel disabled={false} onApply={() => undefined} snapshot={snapshotWithDrift(null)} />)
    expect(markup).toContain("Receiver-ingress drift: Unknown")
    expect(markup).not.toContain("0 ppm")
  })

  it("labels regression statistics without claiming a clock bound or capture latency", () => {
    const markup = renderToStaticMarkup(<SyncPanel disabled={false} onApply={() => undefined} snapshot={snapshotWithDrift({ samples: "1800", timestampRatePpm: 12.5, rateStandardErrorPpm: 0.75, arrivalSkewNs: "-42000", observationWindowNanoseconds: "61000000000", timestampResidualRms: "1700000", discontinuities: "2" })} />)
    expect(markup).toContain("Receiver-ingress timestamp rate")
    expect(markup).toContain("Regression standard error")
    expect(markup).toContain("Timestamp residual RMS")
    expect(markup).toContain("not capture latency")
    expect(markup).toContain("not a conservative clock bound")
  })
})
