import { Ear, Plus, Trash, VideoCamera } from "@phosphor-icons/react"
import { useState } from "react"
import type { Snapshot } from "../api/schema"
import type { NewSourcePayload } from "./InputLibraryForms"
import { ConfigureSourceForm, CreateSourceForm } from "./InputLibraryForms"
import { Button, EmptyState, StatusChip } from "./Primitives"
import type { OperatorCommand, OperatorCommandReceipt } from "./ProductionSetup"

type Source = Snapshot["project"]["sources"][number]

export type InputLibraryProps = {
  readonly snapshot: Snapshot
  readonly disabled: boolean
  readonly listeningTrackId: string | null
  readonly onListen: (trackId: string | null) => void
  readonly onCommand: OperatorCommand
}

export function buildSourceConfiguration(source: Source, update: NewSourcePayload) {
  return {
    id: source.id,
    name: update.name,
    enabled: source.enabled,
    adapter: update.adapter,
    tracks: source.tracks.map((track) => ({ id: track.id, kind: track.kind, correctionMs: track.correctionMs })),
    recordOnConnect: source.recordOnConnect,
  }
}

export function videoPlacementRequest(input: { readonly sourceId: string; readonly sceneId: string; readonly sceneRevision: string }) {
  return { targetId: input.sceneId, payload: { sourceId: input.sourceId }, expectedRevision: input.sceneRevision }
}

function receiptFailure(receipt: OperatorCommandReceipt | null): string | null {
  if (receipt === null) return "The command did not complete. Refresh authoritative state before retrying."
  if (receipt.status === "rejected" || receipt.status === "failed") return receipt.reason ?? "The server rejected the command."
  return null
}

export function InputLibrary({ snapshot, disabled, listeningTrackId, onListen, onCommand }: InputLibraryProps) {
  const [expandedSourceId, setExpandedSourceId] = useState<string | null>(null)
  const [message, setMessage] = useState<string | null>(null)
  const scene = snapshot.project.scenes.find((candidate) => candidate.id === snapshot.preview.sceneId)

  const execute = async (input: { readonly kind: "source.create" | "source.configure" | "source.delete" | "scene.item.add"; readonly targetId: string; readonly payload: Readonly<Record<string, unknown>>; readonly revision?: string }): Promise<void> => {
    const receipt = await onCommand(input.kind, input.targetId, input.payload, input.revision)
    if (input.kind === "scene.item.add" && receipt?.status === "applied" && receipt.actualRevision !== undefined) {
      await onCommand("scene.cue", input.targetId, {}, receipt.actualRevision)
    }
    const failure = receiptFailure(receipt)
    setMessage(failure ?? "Applied to authoritative production state.")
  }

  return (
    <div className="production-setup__stack">
      <section className="production-setup__section"><header><div><h3>Create input</h3><p>SRT and synthetic inputs create stable video and audio tracks. Audio remains unrouted.</p></div></header><CreateSourceForm disabled={disabled} onSubmit={(payload) => execute({ kind: "source.create", targetId: snapshot.project.id, payload, revision: snapshot.project.revision })} /></section>
      <section className="production-setup__section">
        <header><div><h3>Input library</h3><p>Inspect tracks, place video independently, listen before mix, or apply shared source configuration.</p></div></header>
        {snapshot.project.sources.length === 0 ? <EmptyState detail="Create an SRT listener or synthetic fixture to begin." title="No inputs configured" /> : snapshot.project.sources.map((source) => (
          <article className="production-setup__card" key={source.id}>
            <div className="production-setup__card-header"><VideoCamera aria-hidden="true" /><div className="row-copy"><strong>{source.name}</strong><small>{source.adapter.kind.toUpperCase()} · {source.dependencyCount} scene placements</small></div><StatusChip tone={source.tracks.some((track) => track.connection === "connected") ? "healthy" : "warning"}>{source.tracks.some((track) => track.connection === "connected") ? "CONNECTED" : "WAITING"}</StatusChip></div>
            <div className="production-setup__track-list">{source.tracks.map((track) => <div className="production-setup__track" key={track.id}><span className="layer-kind">{track.kind}</span><div className="row-copy"><strong>{track.format}</strong><small>{track.connection} · buffer {track.bufferMs} ms · correction {track.correctionMs} ms</small></div>{track.kind === "audio" ? <Button icon={Ear} onClick={() => onListen(listeningTrackId === track.id ? null : track.id)} tone={listeningTrackId === track.id ? "info" : "neutral"}>{listeningTrackId === track.id ? "Listening privately" : "Listen before mix"}</Button> : null}</div>)}</div>
            <div className="production-setup__actions">
              <Button disabled={disabled || scene === undefined || !source.tracks.some((track) => track.kind === "video")} icon={Plus} onClick={() => { if (scene === undefined) return; const request = videoPlacementRequest({ sourceId: source.id, sceneId: scene.id, sceneRevision: scene.revision }); void execute({ kind: "scene.item.add", targetId: request.targetId, payload: request.payload, revision: request.expectedRevision }) }} tone="preview">Add video to Preview</Button>
              <Button disabled={disabled} onClick={() => setExpandedSourceId(expandedSourceId === source.id ? null : source.id)}>{expandedSourceId === source.id ? "Close configuration" : "Configure source"}</Button>
              <Button disabled={disabled || source.dependencyCount > 0} icon={Trash} onClick={() => void execute({ kind: "source.delete", targetId: source.id, payload: {}, revision: snapshot.project.revision })} tone="danger">Delete source</Button>
            </div>
            {source.dependencyCount > 0 ? <p className="production-setup__hint">Remove all {source.dependencyCount} scene placements before deleting this source.</p> : null}
            {expandedSourceId === source.id ? <ConfigureSourceForm disabled={disabled} key={`${source.id}-${source.adapter.kind}`} onSubmit={(payload) => execute({ kind: "source.configure", targetId: source.id, payload: { source: buildSourceConfiguration(source, payload) }, revision: snapshot.project.revision })} source={source} /> : null}
          </article>
        ))}
      </section>
      {message === null ? null : <p aria-live="polite" className="production-setup__message">{message}</p>}
    </div>
  )
}
