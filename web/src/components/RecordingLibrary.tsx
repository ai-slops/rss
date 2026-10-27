import { HardDrives, Warning } from "@phosphor-icons/react"
import type { Snapshot } from "../api/schema"
import { EmptyState, StatusChip } from "./Primitives"

export type RecordingLibraryProps = { readonly snapshot: Snapshot }

export function RecordingLibrary({ snapshot }: RecordingLibraryProps) {
  return (
    <div className="production-setup__stack">
      <div className="production-setup__storage"><HardDrives aria-hidden="true" /><div><strong>{snapshot.storage.freeBytes} bytes free</strong><small>of {snapshot.storage.totalBytes} bytes · {snapshot.storage.writable ? "storage writable" : "storage read only"}</small></div></div>
      <p className="production-setup__hint"><Warning aria-hidden="true" /> Downloads are unavailable because the server does not expose an authenticated recording-download route. Recording paths are never shown in the browser.</p>
      {snapshot.recordings.length === 0 ? <EmptyState detail="Configured recorders expose completed and partial segments here." title="No recording segments" /> : <div className="production-setup__recordings">{snapshot.recordings.map((recording) => (
        <article className="production-setup__card" key={recording.id}>
          <div className="production-setup__card-header"><div className="row-copy"><strong>{recording.name}</strong><small>{recording.bytes} bytes · output {recording.outputId}</small></div><StatusChip tone={recording.validation === "verified" ? "healthy" : recording.validation === "failed" ? "danger" : "warning"}>{recording.validation.toUpperCase()}</StatusChip></div>
          <dl className="production-setup__facts"><div><dt>Started</dt><dd>{recording.startNs}</dd></div><div><dt>Ended</dt><dd>{recording.endNs ?? "Recording or incomplete"}</dd></div><div><dt>SHA-256</dt><dd>{recording.sha256 ?? "Not verified"}</dd></div></dl>
          {recording.gaps.length === 0 ? <p className="production-setup__hint">No recorded gaps reported.</p> : <div><strong>Known gaps</strong><ul>{recording.gaps.map((gap) => <li key={gap}>{gap}</li>)}</ul></div>}
        </article>
      ))}</div>}
    </div>
  )
}
