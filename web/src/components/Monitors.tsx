import { ArrowCounterClockwise, Broadcast, MonitorPlay, Warning } from "@phosphor-icons/react"
import type { Scene, Snapshot } from "../api/schema"
import { Button, StatusChip } from "./Primitives"

type MonitorProps = {
  readonly label: "PREVIEW" | "PROGRAM"
  readonly scene: Scene | undefined
  readonly revision: string
  readonly mediaState: string
}

function Monitor({ label, scene, revision, mediaState }: MonitorProps) {
  const tone = label === "PREVIEW" ? "preview" : "program"
  return (
    <section className={`monitor monitor--${tone}`} aria-label={`${label} monitor`}>
      <header><strong>{label}</strong><span>{scene?.name ?? "Unavailable"}</span></header>
      <div className="monitor__frame">
        <MonitorPlay aria-hidden="true" weight="thin" />
        <div className="monitor__state"><StatusChip tone={mediaState === "live" ? "healthy" : "warning"}>{mediaState.toUpperCase()}</StatusChip><span>{mediaState === "live" ? "Server-rendered media" : "Media image is not current"}</span></div>
      </div>
      <footer><span>REV {revision}</span><span>Display confirmation unavailable</span></footer>
    </section>
  )
}

type MonitorsProps = {
  readonly snapshot: Snapshot
  readonly selectedScene: Scene | undefined
  readonly disabled: boolean
  readonly pendingTake: boolean
  readonly onTake: () => void
  readonly onCuePrevious: () => void
}

export function Monitors({ snapshot, selectedScene, disabled, pendingTake, onTake, onCuePrevious }: MonitorsProps) {
  const programScene = snapshot.project.scenes.find((scene) => scene.id === snapshot.program.sceneId)
  const exact = selectedScene?.revision === snapshot.program.sceneRevision && selectedScene?.id === snapshot.program.sceneId
  return (
    <section className="switching-area">
      <div className="monitor-grid">
        <Monitor label="PREVIEW" mediaState={snapshot.preview.mediaState} revision={selectedScene?.revision ?? "-"} scene={selectedScene} />
        <Monitor label="PROGRAM" mediaState={snapshot.program.health === "healthy" ? "live" : snapshot.program.health} revision={snapshot.program.programRevision} scene={programScene} />
      </div>
      <div className="switching-bar">
        <div className="switching-bar__status">
          {exact ? <StatusChip tone="healthy">MATCHES PROGRAM</StatusChip> : <StatusChip tone="preview">DRAFT DIFFERS</StatusChip>}
          {!exact && selectedScene?.name.toLowerCase().includes("break") ? <span className="audio-warning"><Warning aria-hidden="true" /> Current audio mix will continue</span> : null}
        </div>
        <div className="cluster">
          <Button disabled={disabled || snapshot.program.previousSceneId === null} icon={ArrowCounterClockwise} onClick={onCuePrevious}>Cue previous</Button>
          <Button disabled={disabled || selectedScene === undefined || exact} icon={Broadcast} onClick={onTake} pending={pendingTake} tone="program">TAKE EXACT REVISION</Button>
        </div>
      </div>
    </section>
  )
}
