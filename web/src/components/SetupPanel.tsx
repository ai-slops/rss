import { FileArrowUp, HardDrives, Package, PlugsConnected, Plus, VideoCamera } from "@phosphor-icons/react"
import { useState } from "react"
import type { CommandKind, Snapshot } from "../api/schema"
import { Button, EmptyState, Panel, StatusChip } from "./Primitives"

type SetupPanelProps = {
  readonly snapshot: Snapshot
  readonly disabled: boolean
  readonly onCommand: (kind: CommandKind, targetId: string, payload: Readonly<Record<string, unknown>>, revision?: string) => void
}

type SetupTab = "inputs" | "assets" | "recordings" | "plugins"

export function SetupPanel({ snapshot, disabled, onCommand }: SetupPanelProps) {
  const [tab, setTab] = useState<SetupTab>("inputs")
  const addSourceToPreview = (sourceId: string): void => {
    const source = snapshot.project.sources.find((candidate) => candidate.id === sourceId)
    const track = source?.tracks.find((candidate) => candidate.kind === "video")
    const scene = snapshot.project.scenes.find((candidate) => candidate.id === snapshot.preview.sceneId)
    if (source === undefined || track === undefined || scene === undefined) return
    onCommand("scene.save", scene.id, {
      scene: {
        ...scene,
        items: [...scene.items, {
          id: crypto.randomUUID(),
          name: source.name,
          visible: true,
          locked: false,
          rect: { x: 0, y: 0, width: snapshot.project.canvas.width, height: snapshot.project.canvas.height },
          crop: { left: 0, top: 0, right: 0, bottom: 0 },
          fit: "contain",
          opacity: 1,
          content: { kind: "track", trackId: track.id },
        }],
      },
    }, scene.revision)
  }
  return (
    <Panel actions={<nav className="setup-tabs" aria-label="Setup sections">{(["inputs", "assets", "recordings", "plugins"] as const).map((item) => <button aria-current={tab === item ? "page" : undefined} key={item} onClick={() => setTab(item)} type="button">{item}</button>)}</nav>} title="Setup and library">
        {tab === "inputs" ? <div className="setup-list">{snapshot.project.sources.map((source) => <section className="setup-row" key={source.id}><VideoCamera aria-hidden="true" /><div className="row-copy"><strong>{source.name}</strong><small>{source.adapter.kind} · {source.tracks.length} tracks · {source.dependencyCount} dependencies</small></div><StatusChip tone={source.tracks.some((track) => track.connection === "active") ? "healthy" : "warning"}>{source.tracks.some((track) => track.connection === "active") ? "MEDIA ACTIVE" : "WAITING"}</StatusChip><Button disabled={disabled || !source.tracks.some((track) => track.kind === "video")} icon={Plus} onClick={() => addSourceToPreview(source.id)}>Add video</Button></section>)}<Button disabled={disabled} icon={PlugsConnected} onClick={() => onCommand("source.create", snapshot.project.id, { adapter: { kind: "srt" }, name: "New SRT input" })} tone="preview">Provision SRT input</Button></div> : null}
      {tab === "assets" ? <div className="setup-list">{snapshot.project.assets.length === 0 ? <EmptyState detail="Upload static images used by standby and break scenes." title="No assets" /> : snapshot.project.assets.map((asset) => <section className="setup-row" key={asset.id}><FileArrowUp aria-hidden="true" /><div className="row-copy"><strong>{asset.name}</strong><small>{asset.mime} · version {asset.version}</small></div><span className="telemetry">{asset.path}</span></section>)}<Button disabled={disabled} icon={FileArrowUp}>Upload asset</Button></div> : null}
      {tab === "recordings" ? <div className="setup-list"><div className="storage-banner"><HardDrives aria-hidden="true" /><div><strong>{snapshot.storage.freeBytes} free</strong><small>of {snapshot.storage.totalBytes} · {snapshot.storage.writable ? "writable" : "read only"}</small></div></div>{snapshot.recordings.length === 0 ? <EmptyState detail="Completed and partial recording segments remain visible here." title="No recordings yet" /> : snapshot.recordings.map((recording) => <section className="setup-row" key={recording.id}><CircleIcon validation={recording.validation} /><div className="row-copy"><strong>{recording.name}</strong><small>{recording.bytes} bytes · {recording.validation}</small></div><span className="telemetry">{recording.endNs ?? "Recording"}</span></section>)}</div> : null}
      {tab === "plugins" ? <div className="setup-list">{snapshot.plugins.length === 0 ? <EmptyState detail="Trusted plugins registered by the server appear here. Missing plugin settings are preserved." title="No plugins registered" /> : snapshot.plugins.map((plugin) => <section className="setup-row" key={plugin.id}><Package aria-hidden="true" /><div className="row-copy"><strong>{plugin.name}</strong><small>{plugin.status}</small></div><StatusChip tone={plugin.available ? plugin.enabled ? "healthy" : "neutral" : "danger"}>{plugin.available ? plugin.enabled ? "ENABLED" : "DISABLED" : "UNAVAILABLE"}</StatusChip><Button disabled={disabled || !plugin.available} onClick={() => onCommand("plugin.command", plugin.id, { command: plugin.enabled ? "disable" : "enable" })}>{plugin.enabled ? "Disable" : "Enable"}</Button></section>)}</div> : null}
    </Panel>
  )
}

function CircleIcon({ validation }: { readonly validation: string }) {
  return <span className={`recording-dot ${validation === "verified" ? "is-verified" : ""}`} aria-label={validation} role="img" />
}
