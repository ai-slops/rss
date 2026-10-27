import { ArrowsClockwise, CloudSlash, Gear, SignOut, SpeakerHigh, SpeakerSlash } from "@phosphor-icons/react"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import type { Command, CommandKind, SceneItem, Snapshot } from "../api/schema"
import type { ConnectionState } from "../state/useProduction"
import { AudioMixer } from "./AudioMixer"
import { Inspector } from "./Inspector"
import { Monitors } from "./Monitors"
import { OutputsPanel } from "./OutputsPanel"
import { IconButton, Notice, StatusChip } from "./Primitives"
import { ScenePanel } from "./ScenePanel"
import { SetupPanel } from "./SetupPanel"
import { SyncPanel } from "./SyncPanel"

type ProductionWorkspaceProps = {
  readonly snapshot: Snapshot
  readonly connection: ConnectionState
  readonly error: string | null
  readonly pending: ReadonlySet<string>
  readonly signOut: () => Promise<void>
  readonly refresh: () => Promise<void>
  readonly command: (command: Omit<Command, "id">) => Promise<unknown>
}

type OperationTab = "audio" | "sync" | "outputs" | "setup"

export function ProductionWorkspace(props: ProductionWorkspaceProps) {
  const [sceneId, setSceneId] = useState(props.snapshot.preview.sceneId)
  const [itemId, setItemId] = useState<string | null>(null)
  const [operation, setOperation] = useState<OperationTab>("audio")
  const [listeningTrackId, setListeningTrackId] = useState<string | null>(null)
  const [monitorMuted, setMonitorMuted] = useState(true)
  const undoStack = useRef<readonly SceneItem[][]>([])
  const redoStack = useRef<readonly SceneItem[][]>([])
  const dispatchCommand = props.command
  const engineRunId = props.snapshot.engineRunId
  const disabled = props.connection !== "connected"
  const scene = props.snapshot.project.scenes.find((candidate) => candidate.id === sceneId)
  const item = scene?.items.find((candidate) => candidate.id === itemId)

  useEffect(() => {
    if (props.snapshot.project.scenes.some((candidate) => candidate.id === sceneId)) return
    setSceneId(props.snapshot.preview.sceneId)
  }, [props.snapshot, sceneId])

  const runCommand = useCallback((kind: CommandKind, targetId: string, payload: Readonly<Record<string, unknown>>, expectedRevision?: string): void => {
    const command: Omit<Command, "id"> = {
      kind,
      targetId,
      expectedEngineRunId: engineRunId,
      payload,
      ...(expectedRevision === undefined ? {} : { expectedRevision }),
    }
    void dispatchCommand(command)
  }, [dispatchCommand, engineRunId])

  const patchItem = useCallback((nextItem: SceneItem): void => {
    if (scene === undefined) return
    undoStack.current = [...undoStack.current, scene.items]
    redoStack.current = []
    runCommand("scene.save", scene.id, { scene: { ...scene, items: scene.items.map((candidate) => candidate.id === nextItem.id ? nextItem : candidate) } }, scene.revision)
  }, [runCommand, scene])

  const shortcutState = useRef({ item, patchItem, runCommand, scene })
  useEffect(() => {
    shortcutState.current = { item, patchItem, runCommand, scene }
  }, [item, patchItem, runCommand, scene])

  useEffect(() => {
    const handleShortcut = (event: KeyboardEvent): void => {
      const current = shortcutState.current
      const target = event.target
      if (!(target instanceof HTMLElement) || event.isComposing || target.closest("input, textarea, select, [contenteditable='true'], [role='dialog']") !== null) return
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "z" && current.scene !== undefined) {
        event.preventDefault()
        const from = event.shiftKey ? redoStack : undoStack
        const to = event.shiftKey ? undoStack : redoStack
        const items = from.current.at(-1)
        if (items === undefined) return
        from.current = from.current.slice(0, -1)
        to.current = [...to.current, current.scene.items]
        current.runCommand("scene.save", current.scene.id, { scene: { ...current.scene, items } }, current.scene.revision)
        return
      }
      if (event.key === "Escape") {
        setItemId(null)
        return
      }
      if (current.item === undefined || current.item.locked || !["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(event.key)) return
      event.preventDefault()
      const distance = event.shiftKey ? 10 : 1
      const x = current.item.rect.x + (event.key === "ArrowLeft" ? -distance : event.key === "ArrowRight" ? distance : 0)
      const y = current.item.rect.y + (event.key === "ArrowUp" ? -distance : event.key === "ArrowDown" ? distance : 0)
      current.patchItem({ ...current.item, rect: { ...current.item.rect, x, y } })
    }
    window.addEventListener("keydown", handleShortcut)
    return () => window.removeEventListener("keydown", handleShortcut)
  }, [])

  const sourceHealth = useMemo(() => {
    const tracks = props.snapshot.project.sources.flatMap((source) => source.tracks)
    if (tracks.length === 0) return "No inputs"
    return `${tracks.filter((track) => track.connection === "active").length}/${tracks.length} tracks active`
  }, [props.snapshot])

  return (
    <main className="workspace">
      <header className="workspace-header">
        <div className="brand-lockup"><span className="brand-mark">LP</span><div><strong>{props.snapshot.project.name}</strong><small>Project rev {props.snapshot.project.revision}</small></div></div>
        <div className="header-status">
          <StatusChip tone={props.connection === "connected" ? "healthy" : "warning"}>{props.connection.toUpperCase()}</StatusChip>
          <StatusChip tone={props.snapshot.program.health === "healthy" ? "healthy" : "danger"}>PROGRAM {props.snapshot.program.health.toUpperCase()}</StatusChip>
          <span className="telemetry">{sourceHealth}</span>
        </div>
        <div className="header-actions">
          <IconButton aria-label={monitorMuted ? "Enable browser monitoring" : "Mute browser monitoring"} icon={monitorMuted ? SpeakerSlash : SpeakerHigh} onClick={() => setMonitorMuted((current) => !current)} tone={monitorMuted ? "neutral" : "info"} />
          <IconButton aria-label="Refresh state" icon={ArrowsClockwise} onClick={() => void props.refresh()} />
          <IconButton aria-label="Sign out" icon={SignOut} onClick={() => void props.signOut()} />
        </div>
      </header>
      {props.connection === "stale" ? <Notice tone="warning" title="Control connection lost"><CloudSlash aria-hidden="true" /> Production continues on the server. Live mutations are disabled and will not be queued.</Notice> : null}
      {props.error === null ? null : <Notice tone="danger" title="Command not applied">{props.error}</Notice>}
      <div className="production-grid">
        <ScenePanel disabled={disabled} onCommand={runCommand} onSelectItem={setItemId} onSelectScene={(nextId) => { setSceneId(nextId); setItemId(null); undoStack.current = []; redoStack.current = []; runCommand("scene.cue", nextId, {}, props.snapshot.project.scenes.find((candidate) => candidate.id === nextId)?.revision) }} programSceneId={props.snapshot.program.sceneId} scenes={props.snapshot.project.scenes} selectedItemId={itemId} selectedSceneId={sceneId} />
        <Monitors disabled={disabled} onCuePrevious={() => {
          const previous = props.snapshot.program.previousSceneId
          if (previous !== null) {
            setSceneId(previous)
            runCommand("program.cuePrevious", previous, {}, props.snapshot.program.programRevision)
          }
        }} onTake={() => {
          if (scene === undefined) return
          runCommand("scene.take", scene.id, { sceneRevision: scene.revision, expectedProgramRevision: props.snapshot.program.programRevision }, scene.revision)
        }} pendingTake={props.pending.has("scene.take")} selectedScene={scene} snapshot={props.snapshot} />
        <Inspector disabled={disabled} item={item} onPatch={patchItem} onRemove={() => {
          if (scene === undefined || item === undefined) return
          runCommand("scene.save", scene.id, { scene: { ...scene, items: scene.items.filter((candidate) => candidate.id !== item.id) } }, scene.revision)
          setItemId(null)
        }} scene={scene} />
      </div>
      <section className="operations-band">
        <nav className="operation-tabs" aria-label="Production operations">
          {(["audio", "sync", "outputs", "setup"] as const).map((tab) => <button aria-current={operation === tab ? "page" : undefined} key={tab} onClick={() => setOperation(tab)} type="button">{tab === "setup" ? <Gear aria-hidden="true" /> : null}{tab}</button>)}
        </nav>
        <div className="operations-body">
          {operation === "audio" ? <AudioMixer disabled={disabled} listeningTrackId={listeningTrackId} onCommand={runCommand} onListen={setListeningTrackId} snapshot={props.snapshot} /> : null}
          {operation === "sync" ? <SyncPanel disabled={disabled} onApply={(payload) => runCommand("timing.apply", props.snapshot.project.id, payload, props.snapshot.project.revision)} snapshot={props.snapshot} /> : null}
          {operation === "outputs" ? <OutputsPanel disabled={disabled} onCommand={runCommand} snapshot={props.snapshot} /> : null}
          {operation === "setup" ? <SetupPanel disabled={disabled} onCommand={runCommand} snapshot={props.snapshot} /> : null}
        </div>
      </section>
    </main>
  )
}
