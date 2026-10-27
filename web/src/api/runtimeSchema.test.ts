import { describe, expect, it } from "vitest"
import { optionalTrackDriftSchema, publishingEndpointSchema, recordingRuntimeSchema } from "./runtimeSchema"
import { snapshotSchema } from "./schema"

const legacyRecording = {
  id: "recording-a",
  outputId: "output-a",
  name: "Clean camera",
  path: "/private/recording.mkv",
  tapPoint: "clean_source",
  startNs: "unknown",
  endNs: null,
  bytes: "1024",
  validation: "partial",
  sha256: null,
  gaps: ["recording_failed"],
}

describe("runtime API compatibility", () => {
  it("defaults absent legacy recovery fields to truthful unknown and empty states", () => {
    const recording = recordingRuntimeSchema.parse(legacyRecording)

    expect(recording.recoveryStatus).toBe("unknown")
    expect(recording.mappingStatus).toBe("unknown")
    expect(recording.engineRunId).toBeNull()
    expect(recording.provenance).toEqual([])
    expect(recording.provenanceGaps).toEqual([])
    expect(recording.startNs).toBe("unknown")
  })

  it("parses exact measured drift fields including signed arrival skew", () => {
    const drift = optionalTrackDriftSchema.parse({
      samples: "1800",
      timestampRatePpm: 12.5,
      rateStandardErrorPpm: 0.75,
      arrivalSkewNs: "-42000",
      observationWindowNanoseconds: "61000000000",
      timestampResidualRms: "1700000",
      discontinuities: "2",
    })

    expect(drift?.arrivalSkewNs).toBe("-42000")
    expect(drift?.rateStandardErrorPpm).toBe(0.75)
    expect(optionalTrackDriftSchema.parse(undefined)).toBeNull()
  })

  it("normalizes legacy numeric provenance nanoseconds without assuming BigInt", () => {
    const recording = recordingRuntimeSchema.parse({
      ...legacyRecording,
      recoveryStatus: "recovered",
      mappingStatus: "mapped",
      provenance: [{
        engine_run_id: "run-a",
        source_id: "source-a",
        track_id: "track-a",
        track_kind: "video",
        continuity_generation: 3,
        local_start_ns: 10,
        local_end_ns: 20,
        media_start_ns: 30,
        media_end_ns: 40,
      }],
    })

    expect(recording.provenance[0]?.local_start_ns).toBe("10")
    expect(recording.provenance[0]?.media_end_ns).toBe("40")
  })

  it("parses authenticated publishing stream identity", () => {
    const endpoint = publishingEndpointSchema.parse({
      sourceId: "source-a",
      protocol: "srt",
      bindAddress: "127.0.0.1",
      port: 9000,
      bufferMs: 120,
      passphrase: "private-passphrase",
      streamId: "#!::u=publisher,r=peer-specific",
      pbkeylen: 16,
    })

    expect(endpoint.streamId).toContain("peer-specific")
  })

  it("parses an old shipped snapshot with no drift or recovery extensions", () => {
    const scene = { id: "scene-a", name: "Standby", revision: "1", items: [] }
    const parsed = snapshotSchema.parse({
      cursor: "1", serverTime: "now", engineRunId: "run-a",
      project: {
        id: "project-a", name: "Production", revision: "1", canvas: { width: 960, height: 540, fpsNumerator: 30, fpsDenominator: 1 }, scenes: [scene],
        sources: [{ id: "source-a", name: "Camera", enabled: true, adapter: { kind: "srt", port: 9000, bufferMs: 120 }, tracks: [{ id: "track-a", kind: "video", correctionMs: 0, connection: "connected", format: "video", bufferMs: 0, lateFrames: 0 }], recordOnConnect: false, dependencyCount: 0 }],
        audioRoutes: [], holdbackMs: 500, outputs: [], assets: [],
      },
      preview: { sceneId: "scene-a", scene, revision: "1", renderedRevision: null, visibleRevision: null, mediaState: "live" },
      program: { sceneId: "scene-a", appliedScene: scene, sceneRevision: "1", programRevision: "1", appliedAtNs: "0", health: "healthy", previousSceneId: null },
      outputStatuses: [], recordings: [legacyRecording], plugins: [], storage: { freeBytes: "1", totalBytes: "2", writable: true },
    })

    expect(parsed.project.sources[0]?.tracks[0]?.drift).toBeNull()
    expect(parsed.recordings[0]?.recoveryStatus).toBe("unknown")
    expect(parsed.project.sources[0]?.adapter).toMatchObject({ bindAddress: "0.0.0.0" })
  })
})
