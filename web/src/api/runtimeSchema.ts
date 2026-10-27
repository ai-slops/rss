import { z } from "zod"

const unsignedDecimalString = z.string().regex(/^\d+$/u)
const signedDecimalString = z.string().regex(/^-?\d+$/u)
const legacyUnsignedNumber = z.number().int().nonnegative().safe().transform(String)

const exactUnsignedDecimal = z.union([unsignedDecimalString, legacyUnsignedNumber])
const generation = z.union([unsignedDecimalString, legacyUnsignedNumber])

export const trackDriftSchema = z.object({
  samples: unsignedDecimalString,
  timestampRatePpm: z.number().finite(),
  rateStandardErrorPpm: z.number().finite().nonnegative(),
  arrivalSkewNs: signedDecimalString,
  observationWindowNanoseconds: unsignedDecimalString,
  timestampResidualRms: unsignedDecimalString,
  discontinuities: unsignedDecimalString,
})

export const optionalTrackDriftSchema = trackDriftSchema.nullable().optional().default(null)

const provenanceIntervalSchema = z.object({
  engine_run_id: z.string().min(1),
  source_id: z.string().min(1),
  track_id: z.string().min(1),
  track_kind: z.enum(["video", "audio"]),
  continuity_generation: generation,
  local_start_ns: exactUnsignedDecimal,
  local_end_ns: exactUnsignedDecimal,
  media_start_ns: exactUnsignedDecimal,
  media_end_ns: exactUnsignedDecimal,
})

const provenanceGapSchema = z.object({
  source_id: z.string().min(1),
  track_id: z.string().min(1),
  from_generation: generation,
  to_generation: generation,
  local_start_ns: exactUnsignedDecimal,
  local_end_ns: exactUnsignedDecimal,
  media_start_ns: exactUnsignedDecimal,
  media_end_ns: exactUnsignedDecimal,
})

export const recordingRuntimeSchema = z.object({
  id: z.string().min(1),
  outputId: z.string().min(1),
  name: z.string(),
  path: z.string().default(""),
  tapPoint: z.enum(["program", "clean_source"]),
  sessionId: z.string().default(""),
  recoveryStatus: z.enum(["complete", "recovered", "partial", "unknown"]).optional().default("unknown"),
  mappingStatus: z.enum(["mapped", "unmapped", "unknown"]).optional().default("unknown"),
  startNs: z.string(),
  endNs: z.string().nullable(),
  engineRunId: z.string().nullable().optional().default(null),
  mediaStartNs: z.string().nullable().optional().default(null),
  mediaEndNs: z.string().nullable().optional().default(null),
  mediaUncertaintyNs: z.string().nullable().optional().default(null),
  bytes: unsignedDecimalString,
  validation: z.string(),
  sha256: z.string().nullable(),
  gaps: z.array(z.string()).default([]),
  provenance: z.array(provenanceIntervalSchema).default([]),
  provenanceGaps: z.array(provenanceGapSchema).default([]),
})

export const publishingEndpointSchema = z.object({
  sourceId: z.string().min(1),
  protocol: z.literal("srt"),
  bindAddress: z.string().min(1),
  port: z.number().int().min(1).max(65_535),
  bufferMs: z.number().int().nonnegative(),
  passphrase: z.string().min(1),
  streamId: z.string().min(1),
  pbkeylen: z.number().int().positive(),
})

export type PublishingEndpoint = z.infer<typeof publishingEndpointSchema>
