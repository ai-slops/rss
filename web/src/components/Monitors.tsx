import { ArrowCounterClockwise, Broadcast, Warning } from "@phosphor-icons/react"
import { useCallback, useState } from "react"
import type { Scene, Snapshot } from "../api/schema"
import { type BrowserMediaState, MediaPlayer } from "./MediaPlayer"
import { Button, StatusChip } from "./Primitives"

type MonitorProps = {
  readonly label: "PREVIEW" | "PROGRAM"
  readonly scene: Scene | undefined
  readonly revision: string
  readonly mediaState: string
  readonly muted: boolean
  readonly stream: "preview" | "program"
}

function Monitor({ label, scene, revision, mediaState, muted, stream }: MonitorProps) {
  const tone = label === "PREVIEW" ? "preview" : "program"
  const [browserState, setBrowserState] = useState<BrowserMediaState>("connecting")
  const [frames, setFrames] = useState(0)
  const handleState = useCallback((state: BrowserMediaState) => setBrowserState(state), [])
  const handleFrames = useCallback((count: number) => setFrames(count), [])
  return (
    <section className={`monitor monitor--${tone}`} aria-label={`${label} monitor`}>
      <header><strong>{label}</strong><span>{scene?.name ?? "Unavailable"}</span></header>
      <div className="monitor__frame">
        <MediaPlayer muted={muted} onFrames={handleFrames} onState={handleState} stream={stream} />
        <div className="monitor__state"><StatusChip tone={browserState === "live" ? "healthy" : "warning"}>{browserState.toUpperCase()}</StatusChip><span>{browserState === "live" ? `${frames} decoded frames` : `Server: ${mediaState}`}</span></div>
      </div>
      <footer><span>SERVER REV {revision}</span><span>Frame/revision correlation unavailable</span></footer>
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
  readonly monitorMuted: boolean
}

export function Monitors({ snapshot, selectedScene, disabled, pendingTake, onTake, onCuePrevious, monitorMuted }: MonitorsProps) {
  const programScene = snapshot.project.scenes.find((scene) => scene.id === snapshot.program.sceneId)
  const exact = selectedScene?.revision === snapshot.program.sceneRevision && selectedScene?.id === snapshot.program.sceneId
  return (
    <section className="switching-area">
      <div className="monitor-grid">
        <Monitor label="PREVIEW" mediaState={snapshot.preview.mediaState} muted revision={selectedScene?.revision ?? "-"} scene={selectedScene} stream="preview" />
        <Monitor label="PROGRAM" mediaState={snapshot.program.health === "healthy" ? "live" : snapshot.program.health} muted={monitorMuted} revision={snapshot.program.programRevision} scene={programScene} stream="program" />
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
