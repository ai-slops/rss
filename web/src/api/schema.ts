import { z } from "zod"
import { optionalTrackDriftSchema, recordingRuntimeSchema } from "./runtimeSchema"

const id = z.string().min(1)
const revision = z.string().min(1)

const rectSchema = z.object({
  x: z.number(),
  y: z.number(),
  width: z.number().positive(),
  height: z.number().positive(),
})

const cropSchema = z.object({
  left: z.number().min(0).max(1),
  top: z.number().min(0).max(1),
  right: z.number().min(0).max(1),
  bottom: z.number().min(0).max(1),
})

const contentSchema = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("track"), trackId: id }),
  z.object({ kind: z.literal("solid"), color: z.string() }),
  z.object({ kind: z.literal("text"), text: z.string(), color: z.string(), fontSize: z.number() }),
  z.object({ kind: z.literal("image"), assetId: id, version: revision }),
])

export const sceneItemSchema = z.object({
  id,
  name: z.string(),
  visible: z.boolean(),
  locked: z.boolean(),
  rect: rectSchema,
  crop: cropSchema,
  fit: z.enum(["contain", "cover", "stretch"]),
  opacity: z.number().min(0).max(1),
  content: contentSchema,
})

export const sceneSchema = z.object({
  id,
  name: z.string(),
  revision,
  items: z.array(sceneItemSchema),
})

export const trackConnectionSchema = z.enum(["connected", "connecting", "missing"])

const trackSchema = z.object({
  id,
  kind: z.enum(["video", "audio"]),
  correctionMs: z.number().int(),
  connection: trackConnectionSchema.default("connecting"),
  format: z.string().default("Unknown"),
  bufferMs: z.number().int().nonnegative().default(0),
  lateFrames: z.number().int().nonnegative().default(0),
  drift: optionalTrackDriftSchema,
})

const sourceSchema = z.object({
  id,
  name: z.string(),
  enabled: z.boolean(),
  adapter: z.discriminatedUnion("kind", [
    z.object({ kind: z.literal("synthetic"), pattern: z.string(), frequencyHz: z.number() }),
    z.object({ kind: z.literal("srt"), bindAddress: z.string().default("0.0.0.0"), port: z.number(), bufferMs: z.number() }),
  ]),
  tracks: z.array(trackSchema),
  recordOnConnect: z.boolean(),
  dependencyCount: z.number().int().nonnegative().default(0),
})

export const audioRouteSchema = z.object({
  trackId: id,
  gainDb: z.number(),
  muted: z.boolean(),
  routed: z.boolean(),
  inputPeakDb: z.number().nullable(),
  outputPeakDb: z.number().nullable(),
  clipping: z.boolean().nullable(),
})

const outputSchema = z.object({
  id,
  automatic: z.boolean().optional(),
  name: z.string(),
  adapter: z.discriminatedUnion("kind", [
    z.object({ kind: z.literal("programRecording"), segmentSeconds: z.number() }),
    z.object({ kind: z.literal("sourceRecording"), sourceId: id, segmentSeconds: z.number() }),
    z.object({ kind: z.literal("rtmps"), destinationId: id }),
  ]),
})

const outputStatusSchema = z.object({
  outputId: id,
  desired: z.enum(["running", "stopped"]),
  actual: z.enum(["stopped", "starting", "running", "retrying", "finalizing", "failed"]),
  lastMediaAtNs: z.string().nullable(),
  error: z.string().nullable(),
  platformStatus: z.string(),
})

const assetSchema = z.object({
  id,
  version: revision,
  name: z.string(),
  mime: z.string(),
  path: z.string(),
})

const pluginSchema = z.object({
  id,
  name: z.string(),
  available: z.boolean(),
  enabled: z.boolean(),
  status: z.string(),
})

export const snapshotSchema = z.object({
  cursor: z.string(),
  serverTime: z.string(),
  engineRunId: id,
  project: z.object({
    id,
    name: z.string(),
    revision,
    canvas: z.object({
      width: z.number().int().positive(),
      height: z.number().int().positive(),
      fpsNumerator: z.number().int().positive(),
      fpsDenominator: z.number().int().positive(),
    }),
    scenes: z.array(sceneSchema),
    sources: z.array(sourceSchema),
    audioRoutes: z.array(audioRouteSchema),
    holdbackMs: z.number().int(),
    outputs: z.array(outputSchema),
    assets: z.array(assetSchema),
  }),
  preview: z.object({
    sceneId: id,
    scene: sceneSchema,
    revision,
    renderedRevision: revision.nullable(),
    visibleRevision: revision.nullable(),
    mediaState: z.enum(["connecting", "live", "stale", "disconnected"]),
  }),
  program: z.object({
    sceneId: id,
    appliedScene: sceneSchema,
    sceneRevision: revision,
    programRevision: revision,
    appliedAtNs: z.string(),
    health: z.enum(["healthy", "warning", "failed"]),
    previousSceneId: id.nullable(),
  }),
  outputStatuses: z.array(outputStatusSchema),
  recordings: z.array(recordingRuntimeSchema),
  plugins: z.array(pluginSchema),
  storage: z.object({ freeBytes: z.string(), totalBytes: z.string(), writable: z.boolean() }),
})

export const commandResponseSchema = z.object({
  commandId: id,
  status: z.enum(["accepted", "applied", "rejected", "failed"]),
  acceptedAt: z.string(),
  appliedAt: z.string().optional(),
  actualRevision: revision.optional(),
  reason: z.string().optional(),
  pluginResult: z.unknown().optional(),
})

export const errorResponseSchema = z.object({ code: z.string(), message: z.string() })

export const commandKindSchema = z.enum([
  "scene.create",
  "scene.rename",
  "scene.duplicate",
  "scene.move",
  "scene.delete",
  "scene.save",
  "scene.cue",
  "scene.copyCued",
  "scene.take",
  "scene.item.add",
  "program.cuePrevious",
  "source.create",
  "source.configure",
  "source.enable",
  "source.delete",
  "audio.apply",
  "audio.set",
  "audio.gain.set",
  "audio.mute.set",
  "timing.apply",
  "output.create",
  "output.start",
  "output.stop",
  "output.delete",
  "plugin.command",
  "asset.upload",
])

export const errorCodeSchema = z.enum([
  "unauthorized",
  "forbidden",
  "invalid",
  "conflict",
  "unavailable",
  "internal",
  "revision_conflict",
  "engine_run_changed",
  "dependency_blocked",
  "infeasible_timing",
  "output_reconfiguration_required",
])

export type Snapshot = z.infer<typeof snapshotSchema>
export type Scene = z.infer<typeof sceneSchema>
export type SceneItem = z.infer<typeof sceneItemSchema>
export type CommandResponse = z.infer<typeof commandResponseSchema>
export type CommandKind = z.infer<typeof commandKindSchema>

export type Command = {
  readonly id: string
  readonly kind: CommandKind
  readonly targetId: string
  readonly expectedRevision?: string
  readonly expectedEngineRunId?: string
  readonly payload: Readonly<Record<string, unknown>>
}
