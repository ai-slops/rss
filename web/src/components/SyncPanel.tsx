import { ArrowCounterClockwise, LinkSimple, Warning } from "@phosphor-icons/react"
import { useMemo, useState } from "react"
import type { Snapshot } from "../api/schema"
import { Button, Field, Notice, Panel } from "./Primitives"

type SyncPanelProps = {
  readonly snapshot: Snapshot
  readonly disabled: boolean
  readonly onApply: (payload: Readonly<Record<string, unknown>>) => void
}

export function SyncPanel({ snapshot, disabled, onApply }: SyncPanelProps) {
  const tracks = useMemo(() => snapshot.project.sources.flatMap((source) => source.tracks.map((track) => ({ ...track, sourceName: source.name }))), [snapshot])
  const [pendingHoldback, setPendingHoldback] = useState<number | null>(null)
  const [corrections, setCorrections] = useState<Readonly<Record<string, number>>>({})
  const holdback = pendingHoldback ?? snapshot.project.holdbackMs
  const changed = holdback !== snapshot.project.holdbackMs || tracks.some((track) => (corrections[track.id] ?? track.correctionMs) !== track.correctionMs)
  const frameMs = (snapshot.project.canvas.fpsDenominator / snapshot.project.canvas.fpsNumerator) * 1000

  return (
    <Panel scope="LIVE / SHARED" title="Input sync">
      <Notice tone="warning" title="Shared timing"><Warning aria-hidden="true" /> Applying values affects every current consumer and may hold, skip, or insert media.</Notice>
      <div className="sync-header">
        <Field hint="Common intentional delay for all Program tracks." label="Program holdback">
          <div className="unit-input"><input disabled={disabled} onChange={(event) => setPendingHoldback(event.currentTarget.valueAsNumber)} step="10" type="number" value={holdback} /><span>ms</span></div>
        </Field>
        <span className="frame-readout">1 frame = {frameMs.toFixed(2)} ms</span>
      </div>
      <div className="sync-list">
        {tracks.map((track) => {
          const value = corrections[track.id] ?? track.correctionMs
          return (
            <div className="sync-row" key={track.id}>
              <div><strong>{track.sourceName}</strong><small>{track.kind} · {track.format}</small></div>
              <div className="stepper"><button disabled={disabled} onClick={() => setCorrections((current) => ({ ...current, [track.id]: value - frameMs }))} type="button">-1f</button><input aria-label={`${track.sourceName} ${track.kind} correction`} disabled={disabled} onChange={(event) => setCorrections((current) => ({ ...current, [track.id]: event.currentTarget.valueAsNumber }))} type="number" value={value} /><span>ms</span><button disabled={disabled} onClick={() => setCorrections((current) => ({ ...current, [track.id]: value + frameMs }))} type="button">+1f</button></div>
              <span className="telemetry">buffer {track.bufferMs} ms · late {track.lateFrames}</span>
              <Button disabled={disabled || value === 0} icon={ArrowCounterClockwise} onClick={() => setCorrections((current) => ({ ...current, [track.id]: 0 }))}>Reset</Button>
            </div>
          )
        })}
      </div>
      <div className="panel-footer"><span>{changed ? "Pending timing values have not been applied." : "Pending and applied timing match."}</span><Button disabled={disabled || !changed} icon={LinkSimple} onClick={() => onApply({ holdbackMs: holdback, corrections: tracks.map((track) => ({ trackId: track.id, correctionMs: corrections[track.id] ?? track.correctionMs })) })} tone="warning">Validate and apply</Button></div>
    </Panel>
  )
}
