import { ArrowsClockwise, CloudSlash, Gear, SignOut, SpeakerHigh, SpeakerSlash } from "@phosphor-icons/react"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import type { Command, CommandKind, CommandResponse, Scene, SceneItem, Snapshot } from "../api/schema"
import type { ConnectionState } from "../state/useProduction"
import { AudioMixer } from "./AudioMixer"
import { AudioRoutingPanel } from "./AudioRoutingPanel"
import { Inspector } from "./Inspector"
import { type BrowserMediaState, MediaPlayer } from "./MediaPlayer"
import { Monitors } from "./Monitors"
import { OutputsPanel } from "./OutputsPanel"
import { IconButton, Notice, StatusChip } from "./Primitives"
import { ProductionSetup } from "./ProductionSetup"
import { ScenePanel } from "./ScenePanel"
import { SyncPanel } from "./SyncPanel"

type ProductionWorkspaceProps = {
  readonly snapshot: Snapshot
  readonly connection: ConnectionState
  readonly error: string | null
  readonly pending: ReadonlySet<string>
  readonly signOut: () => Promise<void>
  readonly refresh: () => Promise<void>
  readonly command: (command: Omit<Command, "id">) => Promise<CommandResponse | null>
}

type OperationTab = "audio" | "sync" | "outputs" | "setup"
type HistoryEntry = { readonly items: readonly SceneItem[]; readonly revision: string }

export function ProductionWorkspace(props: ProductionWorkspaceProps) {
  const [sceneId, setSceneId] = useState(props.snapshot.preview.sceneId)
  const [itemId, setItemId] = useState<string | null>(null)
  const [operation, setOperation] = useState<OperationTab>("audio")
  const [listeningTrackId, setListeningTrackId] = useState<string | null>(null)
  const [monitorMuted, setMonitorMuted] = useState(true)
  const [monitorVolume, setMonitorVolume] = useState(0.8)
  const [sourceMonitorState, setSourceMonitorState] = useState<BrowserMediaState>("disconnected")
  const [localDraft, setLocalDraft] = useState<Scene | null>(null)
  const [draftConflict, setDraftConflict] = useState(false)
  const [gestureActive, setGestureActive] = useState(false)
  const [aspectLocked, setAspectLocked] = useState(true)
  const takeInFlight = useRef(false)
  const undoStack = useRef<readonly HistoryEntry[]>([])
  const redoStack = useRef<readonly HistoryEntry[]>([])
  const saveQueue = useRef<Promise<CommandResponse | null> | null>(null)
  const dispatchCommand = props.command
  const engineRunId = props.snapshot.engineRunId
  const disabled = props.connection !== "connected"
  const persistedScene = props.snapshot.project.scenes.find((candidate) => candidate.id === sceneId)
  const cuedScene = props.snapshot.preview.scene.id === sceneId ? props.snapshot.preview.scene : persistedScene
  const scene = localDraft?.id === sceneId ? localDraft : cuedScene
  const historicalCue = scene !== undefined && persistedScene !== undefined && scene.revision !== persistedScene.revision && localDraft === null
  const item = scene?.items.find((candidate) => candidate.id === itemId)

  useEffect(() => {
    if (props.snapshot.project.scenes.some((candidate) => candidate.id === sceneId)) return
    setSceneId(props.snapshot.preview.sceneId)
  }, [props.snapshot, sceneId])

  useEffect(() => {
    if (localDraft === null && !draftConflict) setSceneId(props.snapshot.preview.sceneId)
  }, [props.snapshot.preview.sceneId, localDraft, draftConflict])

  const runCommand = useCallback((kind: CommandKind, targetId: string, payload: Readonly<Record<string, unknown>>, expectedRevision?: string): Promise<CommandResponse | null> => {
    const command: Omit<Command, "id"> = {
      kind,
      targetId,
      expectedEngineRunId: engineRunId,
      payload,
      ...(expectedRevision === undefined ? {} : { expectedRevision }),
    }
    return dispatchCommand(command)
  }, [dispatchCommand, engineRunId])

  const saveItems = useCallback((items: readonly SceneItem[], expected?: string, recordHistory = true): Promise<CommandResponse | null> => {
    if (scene === undefined || disabled || draftConflict || historicalCue) return Promise.resolve(null)
    const previousItems = scene.items
    const nextScene = { ...scene, items: [...items] }
    setLocalDraft(nextScene)
    setDraftConflict(false)
    if (recordHistory) redoStack.current = []
    const prior = saveQueue.current
    const operation = (prior ?? Promise.resolve(null)).then(async (priorResponse) => {
      if (priorResponse !== null && priorResponse.status !== "applied") return priorResponse
      const expectedRevision = priorResponse?.actualRevision ?? expected ?? persistedScene?.revision ?? scene.revision
      const response = await runCommand("scene.save", scene.id, { scene: { ...nextScene, revision: expectedRevision } }, expectedRevision)
      if (response?.status === "applied" && response.actualRevision !== undefined) {
        if (recordHistory) undoStack.current = [...undoStack.current, { items: previousItems, revision: response.actualRevision }]
      }
      return response
    })
    saveQueue.current = operation
    void operation.then((response) => {
      if (saveQueue.current !== operation) return
      saveQueue.current = null
      if (response?.status === "applied") setLocalDraft(null)
      else setDraftConflict(true)
    })
    return operation
  }, [disabled, draftConflict, historicalCue, persistedScene?.revision, runCommand, scene])

  const patchItem = useCallback((nextItem: SceneItem, expectedRevision?: string): void => {
    if (scene !== undefined) void saveItems(scene.items.map((candidate) => candidate.id === nextItem.id ? nextItem : candidate), expectedRevision)
  }, [saveItems, scene])

  const shortcutState = useRef({ item, patchItem, saveItems, scene })
  useEffect(() => {
    shortcutState.current = { item, patchItem, saveItems, scene }
  }, [item, patchItem, saveItems, scene])

  useEffect(() => {
    const handleShortcut = (event: KeyboardEvent): void => {
      const current = shortcutState.current
      const target = event.target
      if (!(target instanceof Element) || event.isComposing || target.closest("input, textarea, select, [contenteditable='true'], [role='dialog']") !== null) return
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "z" && current.scene !== undefined) {
        event.preventDefault()
        const from = event.shiftKey ? redoStack : undoStack
        const to = event.shiftKey ? undoStack : redoStack
        const entry = from.current.at(-1)
        if (entry === undefined || entry.revision !== current.scene.revision || saveQueue.current !== null) return
        const currentItems = current.scene.items
        void current.saveItems(entry.items, entry.revision, false).then((response) => {
          if (response?.status === "applied" && response.actualRevision !== undefined) {
            from.current = from.current.slice(0, -1)
            const next = from.current.at(-1)
            if (next !== undefined) from.current = [...from.current.slice(0, -1), { ...next, revision: response.actualRevision }]
            to.current = [...to.current, { items: currentItems, revision: response.actualRevision }]
          }
        })
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
    return `${tracks.filter((track) => track.connection === "connected").length}/${tracks.length} tracks connected`
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
          <label>Monitor volume<input aria-label="This browser monitor volume" type="range" min="0" max="1" step="0.05" value={monitorVolume} onChange={(event) => setMonitorVolume(event.currentTarget.valueAsNumber)} /></label>
          {listeningTrackId === null ? null : <StatusChip tone={sourceMonitorState === "live" ? "info" : "warning"}>PRIVATE SOURCE {sourceMonitorState.toUpperCase()}</StatusChip>}
          <IconButton aria-label={monitorMuted ? "Enable browser monitoring" : "Mute browser monitoring"} icon={monitorMuted ? SpeakerSlash : SpeakerHigh} onClick={() => setMonitorMuted((current) => !current)} tone={monitorMuted ? "neutral" : "info"} />
          <IconButton aria-label="Refresh state" icon={ArrowsClockwise} onClick={() => void props.refresh()} />
          <IconButton aria-label="Sign out" icon={SignOut} onClick={() => void props.signOut()} />
        </div>
      </header>
      <div className="workspace-notices">
      {props.connection === "stale" ? <Notice tone="warning" title="Control connection lost"><CloudSlash aria-hidden="true" /> Production continues on the server. Live mutations are disabled and will not be queued.</Notice> : null}
      {draftConflict ? <Notice tone="warning" title="Draft conflict"><span>Your unsaved layout remains in this browser; the server version was not overwritten.</span><button type="button" onClick={() => {
        if (localDraft === null) return
        const url = URL.createObjectURL(new Blob([JSON.stringify(localDraft, null, 2)], { type: "application/json" }))
        const link = document.createElement("a"); link.href = url; link.download = `draft-${localDraft.id}.json`; link.click(); URL.revokeObjectURL(url)
      }}>Download local draft</button><button type="button" onClick={() => { setLocalDraft(null); setDraftConflict(false); undoStack.current = []; redoStack.current = []; void props.refresh() }}>Discard local changes and refresh</button></Notice> : null}
      {historicalCue ? <Notice tone="warning" title="Historical Preview snapshot"><span>The saved draft is newer. Take restores this exact cued snapshot without overwriting it. To edit the old layout, </span><button type="button" disabled={disabled} onClick={() => { if (scene !== undefined) void runCommand("scene.copyCued", scene.id, { name: `${scene.name.slice(0, 80)} restored copy` }, scene.revision) }}>Copy cued snapshot</button></Notice> : null}
      {props.error === null ? null : <Notice tone="danger" title="Command not applied">{props.error}</Notice>}
      </div>
      <div className="production-grid">
        <ScenePanel projectId={props.snapshot.project.id} projectRevision={props.snapshot.project.revision} disabled={disabled || draftConflict || saveQueue.current !== null || gestureActive} readOnlyDraft={historicalCue} selectedScene={scene} onCommand={runCommand} onEditItems={(items) => { void saveItems(items) }} onSelectItem={setItemId} onSelectScene={(nextId) => { setSceneId(nextId); setLocalDraft(null); setDraftConflict(false); setItemId(null); undoStack.current = []; redoStack.current = []; void runCommand("scene.cue", nextId, {}, props.snapshot.project.scenes.find((candidate) => candidate.id === nextId)?.revision) }} programSceneId={props.snapshot.program.sceneId} scenes={props.snapshot.project.scenes} selectedItemId={itemId} selectedSceneId={sceneId} />
        <Monitors aspectLocked={aspectLocked} readOnlyDraft={historicalCue} disabled={disabled || draftConflict} onCommitItem={patchItem} onGestureChange={setGestureActive} onSelectItem={setItemId} selectedItemId={itemId} monitorVolume={monitorVolume} monitorMuted={monitorMuted || listeningTrackId !== null} onCuePrevious={() => {
          const previous = props.snapshot.program.previousSceneId
          if (previous !== null) {
            setSceneId(previous)
            void runCommand("program.cuePrevious", previous, {}, props.snapshot.program.programRevision)
          }
        }} onTake={() => {
          if (scene === undefined || saveQueue.current !== null || gestureActive || takeInFlight.current || disabled || draftConflict) return
          takeInFlight.current = true
          void runCommand("scene.take", scene.id, { sceneRevision: scene.revision, expectedProgramRevision: props.snapshot.program.programRevision }, scene.revision).finally(() => { takeInFlight.current = false })
        }} pendingTake={props.pending.has("scene.take")} takeBlocked={saveQueue.current !== null || gestureActive} selectedScene={scene} snapshot={props.snapshot} />
        <Inspector aspectLocked={aspectLocked} canvas={props.snapshot.project.canvas} onAspectLockChange={setAspectLocked} disabled={disabled || draftConflict || historicalCue} item={item} onPatch={patchItem} onRemove={() => {
          if (scene === undefined || item === undefined) return
          void saveItems(scene.items.filter((candidate) => candidate.id !== item.id))
          setItemId(null)
        }} scene={scene} />
      </div>
      <section className="operations-band">
        <nav className="operation-tabs" aria-label="Production operations">
          {(["audio", "sync", "outputs", "setup"] as const).map((tab) => <button aria-current={operation === tab ? "page" : undefined} key={tab} onClick={() => setOperation(tab)} type="button">{tab === "setup" ? <Gear aria-hidden="true" /> : null}{tab}</button>)}
        </nav>
        <div className="operations-body">
          {operation === "audio" ? <><AudioRoutingPanel disabled={disabled} listeningTrackId={listeningTrackId} onCommand={runCommand} onListen={setListeningTrackId} snapshot={props.snapshot} /><AudioMixer disabled={disabled} listeningTrackId={listeningTrackId} onCommand={runCommand} onListen={setListeningTrackId} snapshot={props.snapshot} /></> : null}
          {operation === "sync" ? <SyncPanel disabled={disabled} onApply={(payload) => runCommand("timing.apply", props.snapshot.project.id, payload, props.snapshot.project.revision)} snapshot={props.snapshot} /> : null}
          {operation === "outputs" ? <OutputsPanel disabled={disabled} onCommand={runCommand} snapshot={props.snapshot} /> : null}
          {operation === "setup" ? <ProductionSetup disabled={disabled || historicalCue} listeningTrackId={listeningTrackId} onCommand={runCommand} onListen={setListeningTrackId} snapshot={props.snapshot} /> : null}
        </div>
      </section>
      {listeningTrackId === null ? null : <MediaPlayer audioOnly muted={monitorMuted} volume={monitorVolume} onState={setSourceMonitorState} stream={`source-${listeningTrackId}`} />}
    </main>
  )
}
