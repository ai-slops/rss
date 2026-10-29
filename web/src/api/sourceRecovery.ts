import { z } from "zod"

const decimal = z.string().regex(/^\d+$/u)
const optionalDecimal = decimal.nullable()

export const recoveryReceiverStateSchema = z.enum(["unknown", "warming", "healthy", "stalled", "source_disabled", "controller_offline"])
export const recoveryRetryResultSchema = z.enum(["never", "pending", "started", "failed", "recovered", "exhausted", "cancelled"])

export const sourceRecoveryStatusSchema = z.object({
  schemaVersion: z.literal(1),
  sourceId: z.string().min(1),
  policyVersion: decimal,
  sourceEnabled: z.boolean(),
  policy: z.object({ enabled: z.boolean(), maintenance: z.boolean() }).strict(),
  controller: z.object({
    enrolled: z.boolean(),
    controllerId: z.string().uuid().nullable(),
    online: z.boolean(),
    processEpoch: optionalDecimal,
    localDesired: z.enum(["running", "stopped"]),
  }).strict(),
  receiver: z.object({
    state: recoveryReceiverStateSchema,
    continuityGeneration: optionalDecimal,
    videoFrames: optionalDecimal,
    audioBuffers: optionalDecimal,
    lastVideoPtsNs: optionalDecimal,
  }).strict(),
  retry: z.object({
    attempts: z.number().int().nonnegative(),
    maximumAttempts: z.literal(3),
    nextEligibleAtNs: optionalDecimal,
    lastResult: recoveryRetryResultSchema,
  }).strict(),
  capabilities: z.object({
    publisherRestart: z.literal(true),
    processRestart: z.literal(false),
    freezeDetection: z.literal("decoded_counter_and_pts_progress"),
  }).strict(),
}).strict()

export const sourceRecoveryPolicyUpdateSchema = z.object({
  expectedPolicyVersion: decimal,
  enabled: z.boolean(),
  maintenance: z.boolean(),
}).strict()

export const sourceRecoveryRetrySchema = z.object({ requestId: z.string().uuid() }).strict()

export type SourceRecoveryStatus = z.infer<typeof sourceRecoveryStatusSchema>
export type SourceRecoveryPolicyUpdate = z.infer<typeof sourceRecoveryPolicyUpdateSchema>
