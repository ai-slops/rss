import { describe, expect, it } from "vitest"
import type { Snapshot } from "../api/schema"
import { buildSourceConfiguration, videoPlacementRequest } from "./InputLibrary"
import { parseSourceForm } from "./InputLibraryForms"

type Source = Snapshot["project"]["sources"][number]

const source: Source = {
  id: "source-a",
  name: "Camera A",
  enabled: true,
  adapter: { kind: "srt", bindAddress: "0.0.0.0", port: 9_000, bufferMs: 120 },
  tracks: [
    { id: "video-a", kind: "video", correctionMs: 0, connection: "connected", format: "video", bufferMs: 80, lateFrames: 0, drift: null },
    { id: "audio-a", kind: "audio", correctionMs: 15, connection: "connected", format: "audio", bufferMs: 75, lateFrames: 1, drift: null },
  ],
  recordOnConnect: false,
  dependencyCount: 2,
}

describe("source operator contracts", () => {
  it("parses supported SRT and synthetic forms", () => {
    const srt = parseSourceForm({ kind: "srt", name: "Remote", port: "9100", bufferMs: "180" })
    const synthetic = parseSourceForm({ kind: "synthetic", name: "Bars", pattern: "smpte", frequencyHz: "440" })

    expect(srt).toEqual({ kind: "valid", payload: { name: "Remote", adapter: { kind: "srt", bindAddress: "0.0.0.0", port: 9_100, bufferMs: 180 } } })
    expect(synthetic).toEqual({ kind: "valid", payload: { name: "Bars", adapter: { kind: "synthetic", pattern: "smpte", frequencyHz: 440 } } })
  })

  it("rejects invalid source boundary values", () => {
    expect(parseSourceForm({ kind: "srt", name: "", port: "70000", bufferMs: "-1" }).kind).toBe("invalid")
    expect(parseSourceForm({ kind: "synthetic", name: "Bars", pattern: "invalid", frequencyHz: "0" }).kind).toBe("invalid")
  })

  it("preserves source and track identity while removing runtime-only track fields", () => {
    const configured = buildSourceConfiguration(source, { name: "Camera B", adapter: { kind: "srt", bindAddress: "0.0.0.0", port: 9_200, bufferMs: 240 } })

    expect(configured.id).toBe("source-a")
    expect(configured.tracks).toEqual([
      { id: "video-a", kind: "video", correctionMs: 0 },
      { id: "audio-a", kind: "audio", correctionMs: 15 },
    ])
    expect(configured).not.toHaveProperty("dependencyCount")
  })

  it("requests a fresh server placement that reuses the existing source track", () => {
    expect(videoPlacementRequest({ sourceId: source.id, sceneId: "scene-a", sceneRevision: "7" })).toEqual({
      targetId: "scene-a",
      payload: { sourceId: "source-a" },
      expectedRevision: "7",
    })
  })
})
