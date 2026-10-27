import { Ear, MicrophoneSlash, SpeakerHigh } from "@phosphor-icons/react"
import type { CSSProperties } from "react"
import type { CommandKind, Snapshot } from "../api/schema"
import { Button, EmptyState, Panel, ScopeTag } from "./Primitives"

type AudioMixerProps = {
  readonly snapshot: Snapshot
  readonly disabled: boolean
  readonly listeningTrackId: string | null
  readonly onListen: (trackId: string | null) => void
  readonly onCommand: (kind: CommandKind, targetId: string, payload: Readonly<Record<string, unknown>>) => void
}

type MeterStyle = CSSProperties & { readonly "--meter": string }

function meterStyle(levelDb: number): MeterStyle {
  return { "--meter": `${Math.max(0, Math.min(100, 100 + levelDb))}%` }
}

export function AudioMixer({ snapshot, disabled, listeningTrackId, onListen, onCommand }: AudioMixerProps) {
  if (snapshot.project.audioRoutes.length === 0) {
    return <Panel scope="LIVE / SHARED" title="Audio mixer"><EmptyState detail="Audio tracks remain off air until explicitly routed from Inputs." title="No routed audio" /></Panel>
  }
  return (
    <Panel actions={<ScopeTag scope="THIS BROWSER" />} scope="LIVE / SHARED" title="Audio mixer">
      <div className="mixer-reel">
        {snapshot.project.audioRoutes.map((route) => {
          const track = snapshot.project.sources.flatMap((source) => source.tracks.map((candidate) => ({ ...candidate, sourceName: source.name }))).find((candidate) => candidate.id === route.trackId)
          return (
            <section className={`mixer-strip ${route.clipping ? "is-clipping" : ""}`} key={route.trackId}>
              <header><strong>{track?.sourceName ?? "Unknown"}</strong><small>{track?.format ?? route.trackId}</small></header>
              <div className="meter-pair"><span className="meter-bar" style={meterStyle(route.inputPeakDb)} /><span className="meter-bar" style={meterStyle(route.outputPeakDb)} /><span className="sr-only">Input {route.inputPeakDb} dB, output {route.outputPeakDb} dB</span></div>
              <input aria-label={`${track?.sourceName ?? "Track"} gain`} disabled={disabled || !route.routed} max="12" min="-60" onChange={(event) => onCommand("audio.set", route.trackId, { gainDb: event.currentTarget.valueAsNumber })} step="0.5" type="range" value={route.gainDb} />
              <output>{route.gainDb.toFixed(1)} dB</output>
              <Button disabled={disabled || !route.routed} icon={route.muted ? SpeakerHigh : MicrophoneSlash} onClick={() => onCommand("audio.set", route.trackId, { muted: !route.muted })} tone={route.muted ? "danger" : "neutral"}>{route.muted ? "Unmute" : "Mute"}</Button>
              <Button icon={Ear} onClick={() => onListen(listeningTrackId === route.trackId ? null : route.trackId)} tone={listeningTrackId === route.trackId ? "info" : "neutral"}>{listeningTrackId === route.trackId ? "Listening" : "Listen"}</Button>
            </section>
          )
        })}
      </div>
    </Panel>
  )
}
