import { Ear, LinkSimple } from "@phosphor-icons/react"
import { useEffect, useMemo, useState } from "react"
import type { Snapshot } from "../api/schema"
import { Button, EmptyState, Notice, ScopeTag } from "./Primitives"
import type { OperatorCommand } from "./ProductionSetup"

type Source = Snapshot["project"]["sources"][number]
type Route = Snapshot["project"]["audioRoutes"][number]

export type AudioRoutingPanelProps = {
  readonly snapshot: Snapshot
  readonly disabled: boolean
  readonly listeningTrackId: string | null
  readonly onListen: (trackId: string | null) => void
  readonly onCommand: OperatorCommand
}

export function buildAudioRoutes(input: {
  readonly sources: readonly Source[]
  readonly currentRoutes: readonly Route[]
  readonly selectedTrackIds: ReadonlySet<string>
}) {
  const existing = new Map(input.currentRoutes.map((route) => [route.trackId, route]))
  const seen = new Set<string>()
  return input.sources.flatMap((source) => source.tracks.flatMap((track) => {
    if (track.kind !== "audio" || !input.selectedTrackIds.has(track.id) || seen.has(track.id)) return []
    seen.add(track.id)
    const route = existing.get(track.id)
    return [{ trackId: track.id, gainDb: route?.gainDb ?? 0, muted: route?.muted ?? false, routed: true }]
  }))
}

export function AudioRoutingPanel({ snapshot, disabled, listeningTrackId, onListen, onCommand }: AudioRoutingPanelProps) {
  const audioTracks = useMemo(() => snapshot.project.sources.flatMap((source) => source.tracks.filter((track) => track.kind === "audio").map((track) => ({ ...track, sourceName: source.name }))), [snapshot.project.sources])
  const appliedIds = useMemo(() => snapshot.project.audioRoutes.filter((route) => route.routed).map((route) => route.trackId), [snapshot.project.audioRoutes])
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set(appliedIds))
  const [status, setStatus] = useState<string | null>(null)
  const [pending, setPending] = useState(false)
  const [baseRevision, setBaseRevision] = useState<string | null>(null)

  useEffect(() => { if (baseRevision === null) setSelected(new Set(appliedIds)) }, [appliedIds, baseRevision])
  const changed = appliedIds.length !== selected.size || appliedIds.some((id) => !selected.has(id))

  if (audioTracks.length === 0) return <EmptyState detail="Create an input with an audio track before routing the live mix." title="No audio tracks available" />
  return (
    <div className="production-setup__stack">
      <Notice tone="warning" title="Explicit live routing">Scene visibility never routes or mutes audio. Apply this complete route selection to the shared live mix.</Notice>
      <div className="production-setup__section-header"><div><h3>Available audio tracks</h3><p>Listen is private to this browser and does not change routing.</p></div><ScopeTag scope="LIVE / SHARED" /></div>
      <div className="production-setup__route-list">{audioTracks.map((track) => (
        <label className="production-setup__route" key={track.id}>
          <input checked={selected.has(track.id)} disabled={disabled || pending} onChange={(event) => { const checked = event.currentTarget.checked; setBaseRevision((current) => current ?? snapshot.project.revision); setSelected((current) => { const next = new Set(current); if (checked) next.add(track.id); else next.delete(track.id); return next }) }} type="checkbox" />
          <span className="row-copy"><strong>{track.sourceName}</strong><small>{track.format} · {track.connection}</small></span>
          <Button icon={Ear} onClick={() => onListen(listeningTrackId === track.id ? null : track.id)} tone={listeningTrackId === track.id ? "info" : "neutral"}>{listeningTrackId === track.id ? "Listening privately" : "Listen before mix"}</Button>
        </label>
      ))}</div>
      <div className="production-setup__footer"><span>{changed ? "Pending route selection has not been applied." : "Pending and applied routes match."}</span><Button disabled={disabled || pending || !changed} icon={LinkSimple} onClick={() => {
        setPending(true)
        setStatus(null)
        const routes = buildAudioRoutes({ sources: snapshot.project.sources, currentRoutes: snapshot.project.audioRoutes, selectedTrackIds: selected })
        void onCommand("audio.apply", snapshot.project.id, { routes }, baseRevision ?? snapshot.project.revision).then((receipt) => { setStatus(receipt?.status === "applied" ? "Audio routes applied to the live mix." : receipt?.reason ?? "Audio routes were not applied."); if (receipt?.status === "applied") setBaseRevision(null) }).catch((caught: unknown) => {
          if (caught instanceof Error) setStatus(caught.message)
          else throw caught
        }).finally(() => setPending(false))
      }} pending={pending} tone="warning">Apply routes</Button></div>
      {baseRevision === null ? null : <Button disabled={pending} onClick={() => { setSelected(new Set(appliedIds)); setBaseRevision(null); setStatus(null) }}>Reset to applied routes</Button>}
      {status === null ? null : <p aria-live="polite" className="production-setup__message">{status}</p>}
    </div>
  )
}
