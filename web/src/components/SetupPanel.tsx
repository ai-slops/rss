import { Ear, FileArrowUp, HardDrives, Package, PlugsConnected, Plus, TestTube, VideoCamera } from "@phosphor-icons/react"
import { useState } from "react"
import type { CommandKind, CommandResponse, Snapshot } from "../api/schema"
import { Button, EmptyState, Panel, StatusChip } from "./Primitives"

type SetupPanelProps = {
  readonly snapshot: Snapshot
  readonly disabled: boolean
  readonly listeningTrackId: string | null
  readonly onListen: (trackId: string | null) => void
  readonly onCommand: (kind: CommandKind, targetId: string, payload: Readonly<Record<string, unknown>>, revision?: string) => Promise<CommandResponse | null>
}

type SetupTab = "inputs" | "assets" | "recordings" | "plugins"

export function SetupPanel({ snapshot, disabled, listeningTrackId, onListen, onCommand }: SetupPanelProps) {
  const [tab, setTab] = useState<SetupTab>("inputs")
  const addSourceToPreview = async (sourceId: string): Promise<void> => {
    const source = snapshot.project.sources.find((candidate) => candidate.id === sourceId)
    const scene = snapshot.project.scenes.find((candidate) => candidate.id === snapshot.preview.sceneId)
    if (source === undefined || scene === undefined) return
    const response = await onCommand("scene.item.add", scene.id, { sourceId }, scene.revision)
    if (response?.status === "applied" && response.actualRevision !== undefined) {
      await onCommand("scene.cue", scene.id, {}, response.actualRevision)
    }
  }
  return (
    <Panel actions={<nav className="setup-tabs" aria-label="Setup sections">{(["inputs", "assets", "recordings", "plugins"] as const).map((item) => <button aria-current={tab === item ? "page" : undefined} key={item} onClick={() => setTab(item)} type="button">{item}</button>)}</nav>} title="Setup and library">
        {tab === "inputs" ? <div className="setup-list">{snapshot.project.sources.map((source) => {
          const audioTrack = source.tracks.find((track) => track.kind === "audio")
           return <section className="setup-row" key={source.id}><VideoCamera aria-hidden="true" /><div className="row-copy"><strong>{source.name}</strong><small>{source.adapter.kind} · {source.tracks.length} tracks · {source.dependencyCount} dependencies</small></div><StatusChip tone={source.tracks.some((track) => track.connection === "connected") ? "healthy" : "warning"}>{source.tracks.some((track) => track.connection === "connected") ? "MEDIA ACTIVE" : "WAITING"}</StatusChip><div className="cluster"><Button disabled={disabled || audioTrack === undefined} icon={Ear} onClick={() => onListen(listeningTrackId === audioTrack?.id ? null : (audioTrack?.id ?? null))} tone={listeningTrackId === audioTrack?.id ? "info" : "neutral"}>{listeningTrackId === audioTrack?.id ? "Listening" : "Listen"}</Button><Button disabled={disabled || !source.tracks.some((track) => track.kind === "video")} icon={Plus} onClick={() => void addSourceToPreview(source.id)}>Add video</Button></div></section>
        })}<div className="cluster"><Button disabled={disabled} icon={TestTube} onClick={() => onCommand("source.create", snapshot.project.id, { adapter: { kind: "synthetic", pattern: "smpte", frequencyHz: 440 }, name: "Synthetic bars" })} tone="preview">Add synthetic input</Button><Button disabled={disabled} icon={PlugsConnected} onClick={() => onCommand("source.create", snapshot.project.id, { adapter: { kind: "srt", port: 9000, bufferMs: 120 }, name: "New SRT input" })}>Provision SRT input</Button></div></div> : null}
      {tab === "assets" ? <div className="setup-list">{snapshot.project.assets.length === 0 ? <EmptyState detail="Upload static images used by standby and break scenes." title="No assets" /> : snapshot.project.assets.map((asset) => <section className="setup-row" key={asset.id}><FileArrowUp aria-hidden="true" /><div className="row-copy"><strong>{asset.name}</strong><small>{asset.mime} · version {asset.version}</small></div><span className="telemetry">{asset.path}</span></section>)}<Button disabled={disabled} icon={FileArrowUp}>Upload asset</Button></div> : null}
      {tab === "recordings" ? <div className="setup-list"><div className="storage-banner"><HardDrives aria-hidden="true" /><div><strong>{snapshot.storage.freeBytes} free</strong><small>of {snapshot.storage.totalBytes} · {snapshot.storage.writable ? "writable" : "read only"}</small></div></div>{snapshot.recordings.length === 0 ? <EmptyState detail="Completed and partial recording segments remain visible here." title="No recordings yet" /> : snapshot.recordings.map((recording) => <section className="setup-row" key={recording.id}><CircleIcon validation={recording.validation} /><div className="row-copy"><strong>{recording.name}</strong><small>{recording.tapPoint} · {recording.bytes} bytes · {recording.validation}</small></div><span className="telemetry">{recording.endNs ?? "Recording"}</span></section>)}</div> : null}
      {tab === "plugins" ? <div className="setup-list">{snapshot.plugins.length === 0 ? <EmptyState detail="Trusted plugins registered by the server appear here. Missing plugin settings are preserved." title="No plugins registered" /> : snapshot.plugins.map((plugin) => <section className="setup-row" key={plugin.id}><Package aria-hidden="true" /><div className="row-copy"><strong>{plugin.name}</strong><small>{plugin.status}</small></div><StatusChip tone={plugin.available ? plugin.enabled ? "healthy" : "neutral" : "danger"}>{plugin.available ? plugin.enabled ? "ENABLED" : "DISABLED" : "UNAVAILABLE"}</StatusChip><Button disabled={disabled || !plugin.available} onClick={() => onCommand("plugin.command", plugin.id, { command: plugin.enabled ? "disable" : "enable" })}>{plugin.enabled ? "Disable" : "Enable"}</Button></section>)}</div> : null}
    </Panel>
  )
}

function CircleIcon({ validation }: { readonly validation: string }) {
  return <span className={`recording-dot ${validation === "verified" ? "is-verified" : ""}`} aria-label={validation} role="img" />
}
