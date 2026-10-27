import { HardDrives, Warning } from "@phosphor-icons/react"
import type { Snapshot } from "../api/schema"
import type { Tone } from "./Primitives"
import { EmptyState, StatusChip } from "./Primitives"

export type RecordingLibraryProps = { readonly snapshot: Snapshot }
type Recording = Snapshot["recordings"][number]

function recoveryTone(status: Recording["recoveryStatus"]): Tone {
  if (status === "complete") return "healthy"
  if (status === "partial") return "danger"
  if (status === "recovered") return "warning"
  return "neutral"
}

function validationTone(status: string): Tone {
  if (status === "decoded" || status === "verified") return "healthy"
  if (status === "failed") return "danger"
  return "warning"
}

function mappingTone(status: Recording["mappingStatus"]): Tone {
  if (status === "mapped") return "healthy"
  if (status === "unmapped") return "warning"
  return "neutral"
}

function exactValue(value: string | null): string {
  return value === null || value === "unknown" || value.length === 0 ? "Unknown" : value
}

function RecordingCard({ recording }: { readonly recording: Recording }) {
  return (
    <article className="production-setup__card">
      <div className="production-setup__card-header">
        <div className="row-copy"><strong>{recording.name}</strong><small>{recording.bytes} bytes · {recording.tapPoint === "clean_source" ? "captured clean source" : "Program"}</small></div>
        <div className="production-setup__status-row">
          <StatusChip tone={recoveryTone(recording.recoveryStatus)}>{recording.recoveryStatus.toUpperCase()}</StatusChip>
          <StatusChip tone={validationTone(recording.validation)}>{recording.validation.toUpperCase()}</StatusChip>
          <StatusChip tone={mappingTone(recording.mappingStatus)}>{recording.mappingStatus.toUpperCase()}</StatusChip>
        </div>
      </div>
      {recording.recoveryStatus === "recovered" ? <p className="production-setup__hint">Recovered after an interrupted process. Review validation and gaps before relying on this segment.</p> : null}
      {recording.recoveryStatus === "partial" ? <p className="production-setup__error">Partial recovery: the segment may be incomplete.</p> : null}
      {recording.mappingStatus === "unmapped" ? <p className="production-setup__hint">Media-to-engine clock mapping is unavailable; timestamps remain in their labelled domains.</p> : null}
      <dl className="production-setup__facts">
        <div><dt>Session</dt><dd>{exactValue(recording.sessionId)}</dd></div>
        <div><dt>Wall start</dt><dd>{exactValue(recording.startNs)}</dd></div>
        <div><dt>Wall end</dt><dd>{exactValue(recording.endNs)}</dd></div>
        <div><dt>Engine run</dt><dd>{exactValue(recording.engineRunId)}</dd></div>
        <div><dt>Media interval</dt><dd>{exactValue(recording.mediaStartNs)} – {exactValue(recording.mediaEndNs)} ns</dd></div>
        <div><dt>Mapping uncertainty</dt><dd>{exactValue(recording.mediaUncertaintyNs)} ns</dd></div>
      </dl>
      {recording.gaps.length === 0 ? <p className="production-setup__hint">No recording interruption gaps reported.</p> : <div><strong>Known interruptions</strong><ul>{recording.gaps.map((gap) => <li key={gap}>{gap}</li>)}</ul></div>}
      {recording.provenance.length === 0 ? <p className="production-setup__hint">Captured track-generation provenance: Unknown.</p> : (
        <div><strong>Captured track-generation provenance</strong><ul className="production-setup__provenance">{recording.provenance.map((entry) => (
          <li key={`${entry.track_id}-${entry.continuity_generation}-${entry.local_start_ns}`}>
            <strong>{entry.track_kind} · source {entry.source_id} · track {entry.track_id} · generation {entry.continuity_generation}</strong>
            <span>local {entry.local_start_ns}–{entry.local_end_ns} ns · engine media {entry.media_start_ns}–{entry.media_end_ns} ns · run {entry.engine_run_id}</span>
          </li>
        ))}</ul></div>
      )}
      {recording.provenanceGaps.length === 0 ? null : <div><strong>Track-generation provenance gaps</strong><ul>{recording.provenanceGaps.map((gap) => <li key={`${gap.track_id}-${gap.from_generation}-${gap.to_generation}`}>source {gap.source_id} · track {gap.track_id} · generation {gap.from_generation} → {gap.to_generation} · local {gap.local_start_ns}–{gap.local_end_ns} ns · engine media {gap.media_start_ns}–{gap.media_end_ns} ns</li>)}</ul></div>}
      <p className="production-setup__hint">SHA-256: {recording.sha256 ?? "Not verified"}</p>
    </article>
  )
}

export function RecordingLibrary({ snapshot }: RecordingLibraryProps) {
  return (
    <div className="production-setup__stack">
      <div className="production-setup__storage"><HardDrives aria-hidden="true" /><div><strong>{snapshot.storage.freeBytes} bytes free</strong><small>of {snapshot.storage.totalBytes} bytes · {snapshot.storage.writable ? "storage writable" : "storage read only"}</small></div></div>
      <p className="production-setup__hint"><Warning aria-hidden="true" /> Downloads are unavailable because the server does not expose an authenticated recording-download route. Recording paths remain private.</p>
      {snapshot.recordings.length === 0 ? <EmptyState detail="Configured recorders expose completed, recovered, and partial segments here." title="No recording segments" /> : <div className="production-setup__recordings">{snapshot.recordings.map((recording) => <RecordingCard key={recording.id} recording={recording} />)}</div>}
    </div>
  )
}
