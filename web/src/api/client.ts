import ky, { HTTPError } from "ky"
import { type PublishingEndpoint, publishingEndpointSchema } from "./runtimeSchema"
import {
  type Command,
  type CommandResponse,
  commandResponseSchema,
  errorResponseSchema,
  type Snapshot,
  snapshotSchema,
} from "./schema"
import {
  type SourceRecoveryPolicyUpdate,
  type SourceRecoveryStatus,
  sourceRecoveryPolicyUpdateSchema,
  sourceRecoveryRetrySchema,
  sourceRecoveryStatusSchema,
} from "./sourceRecovery"

export class ApiError extends Error {
  readonly name = "ApiError"

  constructor(
    readonly code: string,
    message: string,
    readonly status: number,
  ) {
    super(message)
  }
}

const api = ky.create({
  prefixUrl: "api/v1",
  credentials: "same-origin",
  timeout: 10_000,
  retry: { limit: 0 },
  headers: { "x-live-production": "1" },
})

async function translateError(error: unknown): Promise<never> {
  if (error instanceof HTTPError) {
    const body = errorResponseSchema.safeParse(await error.response.json())
    if (body.success) {
      throw new ApiError(body.data.code, body.data.message, error.response.status)
    }
    throw new ApiError("http_error", error.response.statusText, error.response.status)
  }
  if (error instanceof Error) {
    throw new ApiError("network_error", error.message, 0)
  }
  throw error
}

export async function login(password: string): Promise<void> {
  try {
    await api.post("auth/login", { json: { password } })
  } catch (error) {
    return translateError(error)
  }
}

export async function logout(): Promise<void> {
  try {
    await api.post("auth/logout")
  } catch (error) {
    return translateError(error)
  }
}

export async function getSnapshot(): Promise<Snapshot> {
  try {
    return snapshotSchema.parse(await api.get("snapshot").json())
  } catch (error) {
    return translateError(error)
  }
}

export async function sendCommand(command: Command): Promise<CommandResponse> {
  try {
    return commandResponseSchema.parse(await api.post("commands", { json: command }).json())
  } catch (error) {
    return translateError(error)
  }
}

export async function getSourcePublishingEndpoint(sourceId: string): Promise<PublishingEndpoint> {
  try {
    return publishingEndpointSchema.parse(await api.get(`sources/${encodeURIComponent(sourceId)}/publishing-endpoint`).json())
  } catch (error) {
    return translateError(error)
  }
}

export async function getSourceRecovery(sourceId: string, signal?: AbortSignal): Promise<SourceRecoveryStatus> {
  try {
    const options = signal === undefined ? {} : { signal }
    return sourceRecoveryStatusSchema.parse(await api.get(`sources/${encodeURIComponent(sourceId)}/recovery`, options).json())
  } catch (error) {
    return translateError(error)
  }
}

export async function updateSourceRecovery(sourceId: string, update: SourceRecoveryPolicyUpdate): Promise<SourceRecoveryStatus> {
  try {
    const body = sourceRecoveryPolicyUpdateSchema.parse(update)
    return sourceRecoveryStatusSchema.parse(await api.put(`sources/${encodeURIComponent(sourceId)}/recovery`, { json: body }).json())
  } catch (error) {
    return translateError(error)
  }
}

export async function retrySourceRecovery(sourceId: string, requestId: string): Promise<SourceRecoveryStatus> {
  try {
    const body = sourceRecoveryRetrySchema.parse({ requestId })
    return sourceRecoveryStatusSchema.parse(await api.post(`sources/${encodeURIComponent(sourceId)}/recovery/retry`, { json: body }).json())
  } catch (error) {
    return translateError(error)
  }
}

export async function revokeSourceRecoveryController(sourceId: string): Promise<SourceRecoveryStatus> {
  try {
    return sourceRecoveryStatusSchema.parse(await api.delete(`sources/${encodeURIComponent(sourceId)}/recovery/controller`).json())
  } catch (error) {
    return translateError(error)
  }
}

export function commandId(): string {
  return crypto.randomUUID()
}

export async function provisionDestination(destinationId: string, url: string): Promise<void> {
  try {
    await api.post("output-destinations", { json: { destinationId, url } })
  } catch (error) {
    return translateError(error)
  }
}
