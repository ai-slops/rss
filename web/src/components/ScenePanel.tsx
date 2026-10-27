import { Copy, Eye, EyeSlash, Lock, LockOpen, Plus, Trash } from "@phosphor-icons/react"
import { useState } from "react"
import type { CommandKind, Scene } from "../api/schema"
import { Button, EmptyState, IconButton, Panel, StatusChip } from "./Primitives"

type ScenePanelProps = {
  readonly scenes: readonly Scene[]
  readonly selectedSceneId: string
  readonly selectedItemId: string | null
  readonly programSceneId: string
  readonly disabled: boolean
  readonly onSelectScene: (sceneId: string) => void
  readonly onSelectItem: (itemId: string) => void
  readonly onCommand: (kind: CommandKind, targetId: string, payload: Readonly<Record<string, unknown>>, revision?: string) => void
}

export function ScenePanel(props: ScenePanelProps) {
  const [mode, setMode] = useState<"scenes" | "layers">("scenes")
  const [newName, setNewName] = useState("")
  const selected = props.scenes.find((scene) => scene.id === props.selectedSceneId)

  return (
    <Panel
      actions={<div className="segmented" role="tablist"><button aria-selected={mode === "scenes"} onClick={() => setMode("scenes")} role="tab" type="button">Scenes</button><button aria-selected={mode === "layers"} onClick={() => setMode("layers")} role="tab" type="button">Layers</button></div>}
      className="scene-panel"
      scope="DRAFT"
      title={mode === "scenes" ? "Scenes" : "Layers"}
    >
      {mode === "scenes" ? (
        <div className="stack">
          <form className="create-row" onSubmit={(event) => {
            event.preventDefault()
            if (newName.trim().length === 0) return
            props.onCommand("scene.create", "project", { name: newName.trim() })
            setNewName("")
          }}>
            <input aria-label="New scene name" disabled={props.disabled} onChange={(event) => setNewName(event.currentTarget.value)} placeholder="New scene" value={newName} />
            <IconButton aria-label="Create scene" disabled={props.disabled || newName.trim().length === 0} icon={Plus} tone="preview" type="submit" />
          </form>
          <div className="item-list" role="listbox" aria-label="Scenes">
            {props.scenes.map((scene) => (
              <button aria-selected={scene.id === props.selectedSceneId} className="scene-row" key={scene.id} onClick={() => props.onSelectScene(scene.id)} role="option" type="button">
                <span className="scene-thumb"><span>{scene.items.length}</span></span>
                <span className="row-copy"><strong>{scene.name}</strong><small>Revision {scene.revision}</small></span>
                {scene.id === props.programSceneId ? <StatusChip tone="program">PROGRAM</StatusChip> : null}
              </button>
            ))}
          </div>
          {selected === undefined ? null : (
            <div className="row-actions">
              <Button disabled={props.disabled} icon={Copy} onClick={() => props.onCommand("scene.duplicate", selected.id, {}, selected.revision)}>Duplicate</Button>
              <Button disabled={props.disabled || selected.id === props.programSceneId} icon={Trash} onClick={() => props.onCommand("scene.delete", selected.id, {}, selected.revision)} tone="danger">Delete</Button>
            </div>
          )}
        </div>
      ) : selected === undefined || selected.items.length === 0 ? (
        <EmptyState detail="Add an existing video track from Inputs to place it in this draft." title="No layers in this scene" />
      ) : (
        <div className="item-list" role="listbox" aria-label="Scene layers">
          {[...selected.items].reverse().map((item) => (
            <div className={`layer-row ${item.id === props.selectedItemId ? "is-selected" : ""}`} key={item.id}>
              <button aria-selected={item.id === props.selectedItemId} className="layer-row__select" onClick={() => props.onSelectItem(item.id)} role="option" type="button"><span className="layer-kind">{item.content.kind}</span><span className="row-copy"><strong>{item.name}</strong><small>{item.locked ? "Locked" : "Editable"}</small></span></button>
                <IconButton aria-label={item.visible ? `Hide ${item.name}` : `Show ${item.name}`} disabled={props.disabled} icon={item.visible ? Eye : EyeSlash} onClick={() => props.onCommand("scene.save", selected.id, { scene: { ...selected, items: selected.items.map((candidate) => candidate.id === item.id ? { ...candidate, visible: !candidate.visible } : candidate) } }, selected.revision)} />
                <IconButton aria-label={item.locked ? `Unlock ${item.name}` : `Lock ${item.name}`} disabled={props.disabled} icon={item.locked ? Lock : LockOpen} onClick={() => props.onCommand("scene.save", selected.id, { scene: { ...selected, items: selected.items.map((candidate) => candidate.id === item.id ? { ...candidate, locked: !candidate.locked } : candidate) } }, selected.revision)} />
            </div>
          ))}
        </div>
      )}
    </Panel>
  )
}
