import { describe, expect, it } from "vitest"
import type { Snapshot } from "../api/schema"
import { buildAudioRoutes } from "./AudioRoutingPanel"

type Source = Snapshot["project"]["sources"][number]
type Route = Snapshot["project"]["audioRoutes"][number]

const sources: readonly Source[] = [{
  id: "source-a",
  name: "Camera A",
  enabled: true,
  adapter: { kind: "synthetic", pattern: "smpte", frequencyHz: 440 },
  tracks: [
    { id: "video-a", kind: "video", correctionMs: 0, connection: "connected", format: "video", bufferMs: 0, lateFrames: 0 },
    { id: "audio-a", kind: "audio", correctionMs: 0, connection: "connected", format: "audio", bufferMs: 0, lateFrames: 0 },
  ],
  recordOnConnect: false,
  dependencyCount: 0,
}]

const routes: readonly Route[] = [{ trackId: "audio-a", gainDb: -6, muted: true, routed: true, inputPeakDb: -20, outputPeakDb: -26, clipping: false }]

describe("audio routing payloads", () => {
  it("includes selected audio tracks once and preserves existing live parameters", () => {
    const result = buildAudioRoutes({ sources, currentRoutes: routes, selectedTrackIds: new Set(["audio-a", "video-a"]) })

    expect(result).toEqual([{ trackId: "audio-a", gainDb: -6, muted: true, routed: true }])
  })

  it("uses neutral defaults for a newly routed microphone", () => {
    const result = buildAudioRoutes({ sources, currentRoutes: [], selectedTrackIds: new Set(["audio-a"]) })

    expect(result).toEqual([{ trackId: "audio-a", gainDb: 0, muted: false, routed: true }])
  })
})
