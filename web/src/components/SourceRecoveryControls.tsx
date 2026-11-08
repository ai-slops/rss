import { ArrowClockwise, ShieldSlash, Trash } from "@phosphor-icons/react"
import { useCallback, useEffect, useRef, useState } from "react"
import { ApiError, getSourceRecovery, retrySourceRecovery, revokeSourceRecoveryController, updateSourceRecovery } from "../api/client"
import type { SourceRecoveryStatus } from "../api/sourceRecovery"
import type { Tone } from "./Primitives"
import { Button, Notice, StatusChip } from "./Primitives"

export type SourceRecoveryControlsProps = { readonly sourceId: string; readonly disabled: boolean }
type PolicyDraft = { readonly version: string; readonly enabled: boolean; readonly maintenance: boolean }
type PendingAction = "policy" | "disable" | "retry" | "revoke" | null

export function canManualRetry(status: SourceRecoveryStatus, disabled: boolean): boolean {
  return !disabled && status.sourceEnabled && status.policy.enabled && !status.policy.maintenance && status.controller.enrolled && status.controller.online && status.controller.localDesired === "running"
}

function receiverTone(state: SourceRecoveryStatus["receiver"]["state"]): Tone {
  if (state === "healthy") return "healthy"
  if (state === "stalled") return "danger"
  if (state === "warming" || state === "controller_offline" || state === "source_disabled") return "warning"
  return "neutral"
}

function receiverLabel(state: SourceRecoveryStatus["receiver"]["state"]): string {
  return state.replaceAll("_", " ").toUpperCase()
}

export function SourceRecoveryControls({ sourceId, disabled }: SourceRecoveryControlsProps) {
  const [status, setStatus] = useState<SourceRecoveryStatus | null>(null)
  const [reachable, setReachable] = useState(false)
  const [draft, setDraft] = useState<PolicyDraft | null>(null)
  const [pending, setPending] = useState<PendingAction>(null)
  const [error, setError] = useState<string | null>(null)
  const dirty = useRef(false)

  const acceptStatus = useCallback((next: SourceRecoveryStatus, clearError = true): void => {
    setStatus(next)
    setReachable(true)
    if (clearError) setError(null)
    if (!dirty.current) setDraft({ version: next.policyVersion, enabled: next.policy.enabled, maintenance: next.policy.maintenance })
  }, [])

  useEffect(() => {
    const controller = new AbortController()
    let fetching = false
    const refresh = async (): Promise<void> => {
      if (fetching) return
      fetching = true
      try {
        const next = await getSourceRecovery(sourceId, controller.signal)
        acceptStatus(next, !dirty.current)
      } catch (caught) {
        if (!controller.signal.aborted) {
          setReachable(false)
          setError(caught instanceof Error ? caught.message : "Recovery status is unavailable.")
        }
      } finally {
        fetching = false
      }
    }
    void refresh()
    const timer = window.setInterval(() => void refresh(), 2_000)
    return () => { window.clearInterval(timer); controller.abort() }
  }, [sourceId, acceptStatus])

  const refreshAfterConflict = async (): Promise<void> => {
    try {
      const next = await getSourceRecovery(sourceId)
      acceptStatus(next, false)
      setDraft((current) => current === null ? null : { ...current, version: next.policyVersion })
    } catch (caught) {
      setReachable(false)
      setError(caught instanceof Error ? caught.message : "Recovery status is unavailable.")
    }
  }

  const runMutation = async (action: Exclude<PendingAction, null>, operation: () => Promise<SourceRecoveryStatus>): Promise<void> => {
    setPending(action)
    setError(null)
    try {
      const next = await operation()
      dirty.current = false
      setDraft({ version: next.policyVersion, enabled: next.policy.enabled, maintenance: next.policy.maintenance })
      acceptStatus(next)
    } catch (caught) {
      if (caught instanceof ApiError && caught.status === 409) {
        setError(action === "policy" || action === "disable" ? "Recovery policy changed on the server. Status was refreshed; review before applying again." : caught.message)
        await refreshAfterConflict()
      } else {
        if (caught instanceof ApiError && caught.status === 0) setReachable(false)
        setError(caught instanceof Error ? caught.message : "Recovery command failed.")
      }
    } finally {
      setPending(null)
    }
  }

  const controlsDisabled = disabled || !reachable || pending !== null
  const updateDraft = (next: Omit<PolicyDraft, "version">): void => {
    if (draft === null) return
    dirty.current = true
    setDraft({ version: draft.version, ...next })
  }

  return (
    <section className="source-recovery" aria-label="Source recovery">
      <div className="source-recovery__heading"><div><h4>Publisher recovery</h4><p>Manual OBS Start arms recovery. Intentional Stop remains stopped and is never reversed.</p></div>{reachable && status !== null ? <StatusChip tone={receiverTone(status.receiver.state)}>{receiverLabel(status.receiver.state)}</StatusChip> : <StatusChip tone="warning">STATUS UNAVAILABLE</StatusChip>}</div>
      {!reachable ? <Notice tone="warning" title="Recovery status unavailable">Previous health is not shown as current. Mutations are disabled until control reconnects.</Notice> : null}
      {status !== null && reachable ? <>
        <dl className="source-recovery__facts"><div><dt>Controller</dt><dd>{status.controller.enrolled ? status.controller.online ? "Enrolled · online" : "Enrolled · offline" : "Not enrolled"}</dd></div><div><dt>Controller epoch</dt><dd>{status.controller.processEpoch ?? "Unknown"}</dd></div><div><dt>Local intent</dt><dd>{status.controller.localDesired}</dd></div><div><dt>Receiver</dt><dd>{receiverLabel(status.receiver.state)}</dd></div><div><dt>Media progress</dt><dd>video {status.receiver.videoFrames ?? "Unknown"} · audio {status.receiver.audioBuffers ?? "Unknown"}</dd></div><div><dt>Retry budget</dt><dd>{status.retry.attempts} / {status.retry.maximumAttempts} · {status.retry.lastResult}</dd></div><div><dt>Next eligibility</dt><dd>{status.retry.nextEligibleAtNs ?? "Eligible now or server-determined"}</dd></div></dl>
        <p className="production-setup__hint">Continuity generation: {status.receiver.continuityGeneration ?? "Unknown"} · last video PTS: {status.receiver.lastVideoPtsNs ?? "Unknown"} ns</p>
      </> : null}
      {draft === null ? null : <fieldset className="source-recovery__policy" disabled={controlsDisabled}><legend>Recovery policy</legend><label><input checked={draft.enabled} onChange={(event) => updateDraft({ enabled: event.currentTarget.checked, maintenance: draft.maintenance })} type="checkbox" /> Enable publisher recovery</label><label><input checked={draft.maintenance} onChange={(event) => updateDraft({ enabled: draft.enabled, maintenance: event.currentTarget.checked })} type="checkbox" /> Maintenance mode</label><div className="production-setup__actions"><Button disabled={controlsDisabled || !dirty.current} onClick={() => void runMutation("policy", () => updateSourceRecovery(sourceId, { expectedPolicyVersion: draft.version, enabled: draft.enabled, maintenance: draft.maintenance }))} pending={pending === "policy"} tone="warning">Apply recovery policy</Button><Button disabled={controlsDisabled || status?.policy.enabled !== true} icon={ShieldSlash} onClick={() => { if (status !== null) void runMutation("disable", () => updateSourceRecovery(sourceId, { expectedPolicyVersion: status.policyVersion, enabled: false, maintenance: status.policy.maintenance })) }} pending={pending === "disable"} tone="danger">Disable recovery</Button></div></fieldset>}
      <div className="production-setup__actions"><Button disabled={status === null || !canManualRetry(status, controlsDisabled)} icon={ArrowClockwise} onClick={() => void runMutation("retry", () => retrySourceRecovery(sourceId, crypto.randomUUID()))} pending={pending === "retry"}>Request publisher retry</Button><Button disabled={controlsDisabled || status?.controller.enrolled !== true} icon={Trash} onClick={() => { if (window.confirm("Revoke this source recovery controller? The existing controller token will stop working.")) void runMutation("revoke", () => revokeSourceRecoveryController(sourceId)) }} pending={pending === "revoke"} tone="danger">Revoke controller</Button></div>
      {status?.controller.enrolled === false ? <p className="production-setup__hint">Enroll privately with the CLI <code>--with-recovery</code> flow. Controller tokens are never displayed in this browser.</p> : null}
      <p className="production-setup__hint">Recovery can restart only the enrolled Publisher inside a running OBS process. A dead OBS process requires separate external supervision or manual intervention, which is unavailable here.</p>
      {error === null ? null : <p className="production-setup__error" role="alert">{error}</p>}
    </section>
  )
}
