import { HardDrives, Image, Microphone, Package, VideoCamera } from "@phosphor-icons/react"
import { useState } from "react"
import type { CommandKind, CommandResponse, Snapshot } from "../api/schema"
import { AssetLibrary } from "./AssetLibrary"
import { AudioRoutingPanel } from "./AudioRoutingPanel"
import { InputLibrary } from "./InputLibrary"
import { PluginLibrary } from "./PluginLibrary"
import { Panel } from "./Primitives"
import { RecordingLibrary } from "./RecordingLibrary"
import "./ProductionSetup.css"

export type OperatorCommandKind = CommandKind

export type OperatorCommandReceipt = CommandResponse

export type OperatorCommand = (
  kind: OperatorCommandKind,
  targetId: string,
  payload: Readonly<Record<string, unknown>>,
  expectedRevision?: string,
) => Promise<OperatorCommandReceipt | null>

export type ProductionSetupProps = {
  readonly snapshot: Snapshot
  readonly disabled: boolean
  readonly listeningTrackId: string | null
  readonly onListen: (trackId: string | null) => void
  readonly onCommand: OperatorCommand
}

const TABS = [
  { id: "inputs", label: "Inputs", icon: VideoCamera },
  { id: "audio", label: "Audio routes", icon: Microphone },
  { id: "assets", label: "Assets", icon: Image },
  { id: "recordings", label: "Recordings", icon: HardDrives },
  { id: "plugins", label: "Extensions", icon: Package },
] as const

type SetupTab = (typeof TABS)[number]["id"]

export function ProductionSetup(props: ProductionSetupProps) {
  const [tab, setTab] = useState<SetupTab>("inputs")
  return (
    <Panel
      actions={(
        <nav aria-label="Setup sections" className="production-setup__tabs">
          {TABS.map(({ id, label, icon: Icon }) => (
            <button aria-current={tab === id ? "page" : undefined} key={id} onClick={() => setTab(id)} type="button">
              <Icon aria-hidden="true" />
              <span>{label}</span>
            </button>
          ))}
        </nav>
      )}
      className="production-setup"
      title="Setup and library"
    >
      {tab === "inputs" ? <InputLibrary {...props} /> : null}
      {tab === "audio" ? <AudioRoutingPanel {...props} /> : null}
      {tab === "assets" ? <AssetLibrary {...props} /> : null}
      {tab === "recordings" ? <RecordingLibrary snapshot={props.snapshot} /> : null}
      {tab === "plugins" ? <PluginLibrary disabled={props.disabled} onCommand={props.onCommand} snapshot={props.snapshot} /> : null}
    </Panel>
  )
}
