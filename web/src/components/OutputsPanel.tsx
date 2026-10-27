import { Broadcast, Circle, Record, Stop } from "@phosphor-icons/react"
import { useState } from "react"
import type { CommandKind, Snapshot } from "../api/schema"
import { Button, EmptyState, Field, Panel, StatusChip } from "./Primitives"

type OutputsPanelProps = {
  readonly snapshot: Snapshot
  readonly disabled: boolean
  readonly onCommand: (kind: CommandKind, targetId: string, payload: Readonly<Record<string, unknown>>) => void
}

export function OutputsPanel({ snapshot, disabled, onCommand }: OutputsPanelProps) {
  const [destinationId, setDestinationId] = useState("")
  const [cleanSourceId, setCleanSourceId] = useState("")
  const selectedSourceId = cleanSourceId || snapshot.project.sources[0]?.id || ""
  const normalizedDestinationId = destinationId.trim()
  const destinationIdValid = /^[A-Za-z0-9._-]{1,64}$/u.test(normalizedDestinationId)

  return (
    <Panel scope="LIVE / SHARED" title="Outputs and recording">
      <div className="output-config-actions row-actions">
        <Button disabled={disabled} icon={Record} onClick={() => onCommand("output.create", snapshot.project.id, { adapter: { kind: "programRecording", segmentSeconds: 300 }, name: "Program recording" })}>Add Program recorder</Button>
        <Field hint="Records the selected source before Program timing and composition." label="Clean source">
          <select disabled={disabled || snapshot.project.sources.length === 0} onChange={(event) => setCleanSourceId(event.currentTarget.value)} value={selectedSourceId}>
            {snapshot.project.sources.length === 0 ? <option value="">No sources available</option> : snapshot.project.sources.map((source) => <option key={source.id} value={source.id}>{source.name}</option>)}
          </select>
          <Button disabled={disabled || selectedSourceId === ""} icon={Record} onClick={() => onCommand("output.create", snapshot.project.id, { adapter: { kind: "sourceRecording", sourceId: selectedSourceId, segmentSeconds: 300 }, name: "Clean source recording" })}>Add clean recorder</Button>
        </Field>
        <Field hint="Credentials remain server-side; enter only an authorized destination ID." label="Provisioned RTMPS destination ID">
          <input disabled={disabled} maxLength={64} onChange={(event) => setDestinationId(event.currentTarget.value)} pattern="[A-Za-z0-9._-]{1,64}" placeholder="destination-id" value={destinationId} />
          <Button disabled={disabled || !destinationIdValid} icon={Broadcast} onClick={() => onCommand("output.create", snapshot.project.id, { adapter: { destinationId: normalizedDestinationId, kind: "rtmps" }, name: "RTMPS destination" })}>Add RTMPS destination</Button>
        </Field>
      </div>
      {snapshot.project.outputs.length === 0 ? <EmptyState detail="Add a Program recorder, clean-source recorder, or RTMPS destination from Setup." title="No outputs configured" /> : (
        <div className="output-list">
          {snapshot.project.outputs.map((output) => {
            const status = snapshot.outputStatuses.find((candidate) => candidate.outputId === output.id)
            const active = status?.desired === "running"
            const publicRisk = output.adapter.kind === "rtmps"
            return (
              <section className="output-row" key={output.id}>
                <span className={`output-icon output-icon--${active ? "active" : "idle"}`}>{output.adapter.kind.includes("Recording") ? <Record aria-hidden="true" weight="fill" /> : <Broadcast aria-hidden="true" />}</span>
                <div className="row-copy"><strong>{output.name}</strong><small>{output.adapter.kind} · desired {status?.desired ?? "unknown"}</small></div>
                <div className="output-state"><StatusChip tone={status?.actual === "running" ? "healthy" : status?.actual === "failed" ? "danger" : status?.actual === "retrying" ? "warning" : "neutral"}>{status?.actual?.toUpperCase() ?? "UNKNOWN"}</StatusChip><small>{status?.platformStatus ?? "Platform status unknown"}</small></div>
                <span className="telemetry">{status?.lastMediaAtNs === null || status?.lastMediaAtNs === undefined ? "No media observed" : `media ${status.lastMediaAtNs}`}</span>
                <Button disabled={disabled} icon={active ? Stop : publicRisk ? Broadcast : Circle} onClick={() => {
                  if (!active && publicRisk && !window.confirm(`Start sending to ${output.name}? Media may become public immediately.`)) return
                  onCommand(active ? "output.stop" : "output.start", output.id, active ? {} : { confirmPublic: publicRisk })
                }} tone={active ? "danger" : publicRisk ? "program" : "neutral"}>{active ? "Stop" : publicRisk ? "Start sending" : "Start"}</Button>
                {status?.error === null || status?.error === undefined ? null : <p className="output-error">{status.error}</p>}
              </section>
            )
          })}
        </div>
      )}
    </Panel>
  )
}
