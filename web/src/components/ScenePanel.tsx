import { ArrowDown, ArrowUp, Copy, Eye, EyeSlash, Lock, LockOpen, Plus, Trash } from "@phosphor-icons/react"
import { useState } from "react"
import type { CommandKind, Scene, SceneItem } from "../api/schema"
import { Button, EmptyState, Field, IconButton, Panel, StatusChip } from "./Primitives"

type ScenePanelProps = {
  readonly scenes: readonly Scene[]
  readonly selectedSceneId: string
  readonly selectedItemId: string | null
  readonly programSceneId: string
  readonly projectId: string
  readonly projectRevision: string
  readonly disabled: boolean
  readonly readOnlyDraft: boolean
  readonly selectedScene: Scene | undefined
  readonly onSelectScene: (sceneId: string) => void
  readonly onSelectItem: (itemId: string) => void
  readonly onEditItems: (items: readonly SceneItem[]) => void
  readonly onCommand: (kind: CommandKind, targetId: string, payload: Readonly<Record<string, unknown>>, revision?: string) => void
}

export function ScenePanel(props: ScenePanelProps) {
  const [mode, setMode] = useState<"scenes" | "layers">("scenes")
  const [newName, setNewName] = useState("")
  const saved = props.scenes.find((scene) => scene.id === props.selectedSceneId)
  const selected = props.selectedScene ?? saved
  const selectedIndex = props.scenes.findIndex((scene) => scene.id === props.selectedSceneId)
  const layerIndex = selected?.items.findIndex((item) => item.id === props.selectedItemId) ?? -1
  const layer = selected?.items[layerIndex]
  const moveLayer = (direction: number): void => {
    const neighbor = selected?.items[layerIndex + direction]
    if (selected === undefined || layer === undefined || layer.locked || neighbor === undefined || neighbor.locked) return
    props.onEditItems(selected.items.map((item, index) => index === layerIndex ? neighbor : index === layerIndex + direction ? layer : item))
  }

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
            props.onCommand("scene.create", props.projectId, { name: newName.trim() }, props.projectRevision)
            setNewName("")
          }}>
            <input aria-label="New scene name" disabled={props.disabled} onChange={(event) => setNewName(event.currentTarget.value)} placeholder="New scene" value={newName} />
            <IconButton aria-label="Create scene" disabled={props.disabled || newName.trim().length === 0} icon={Plus} tone="preview" type="submit" />
          </form>
          <div className="item-list" role="listbox" aria-label="Scenes">
            {props.scenes.map((scene) => (
              <button aria-selected={scene.id === props.selectedSceneId} className="scene-row" disabled={props.disabled} key={scene.id} onClick={() => props.onSelectScene(scene.id)} role="option" type="button">
                <span className="scene-thumb"><span>{scene.items.length}</span></span>
                <span className="row-copy"><strong>{scene.name}</strong><small>Revision {scene.revision}</small></span>
                {scene.id === props.programSceneId ? <StatusChip tone="program">PROGRAM</StatusChip> : null}
              </button>
            ))}
          </div>
          {selected === undefined ? null : (
            <div className="row-actions">
              <Field label="Scene name"><input key={saved?.id} defaultValue={saved?.name} disabled={props.disabled} maxLength={100} onBlur={(event) => { if (saved !== undefined && event.currentTarget.value.trim() !== saved.name && event.currentTarget.value.trim() !== "") props.onCommand("scene.rename", saved.id, { name: event.currentTarget.value.trim() }, saved.revision) }} /></Field>
              <Button disabled={props.disabled} icon={Copy} onClick={() => { if (saved !== undefined) props.onCommand("scene.duplicate", saved.id, {}, saved.revision) }}>Duplicate saved scene</Button>
              <Button disabled={props.disabled || selectedIndex <= 0} icon={ArrowUp} onClick={() => props.onCommand("scene.move", selected.id, { index: selectedIndex - 1 }, props.projectRevision)}>Move scene up</Button>
              <Button disabled={props.disabled || selectedIndex >= props.scenes.length - 1} icon={ArrowDown} onClick={() => props.onCommand("scene.move", selected.id, { index: selectedIndex + 1 }, props.projectRevision)}>Move scene down</Button>
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
                <IconButton aria-label={item.visible ? `Hide ${item.name}` : `Show ${item.name}`} disabled={props.disabled || props.readOnlyDraft} icon={item.visible ? Eye : EyeSlash} onClick={() => props.onEditItems(selected.items.map((candidate) => candidate.id === item.id ? { ...candidate, visible: !candidate.visible } : candidate))} />
                <IconButton aria-label={item.locked ? `Unlock ${item.name}` : `Lock ${item.name}`} disabled={props.disabled || props.readOnlyDraft} icon={item.locked ? Lock : LockOpen} onClick={() => props.onEditItems(selected.items.map((candidate) => candidate.id === item.id ? { ...candidate, locked: !candidate.locked } : candidate))} />
            </div>
          ))}
          {layer === undefined ? null : <div className="row-actions">
            <Button disabled={props.disabled || props.readOnlyDraft || layer.locked} icon={Copy} onClick={() => props.onEditItems([...selected.items, { ...layer, id: crypto.randomUUID(), name: `${layer.name.slice(0, 90)} copy` }])}>Duplicate layer</Button>
            <Button disabled={props.disabled || props.readOnlyDraft || layer.locked || layerIndex === selected.items.length - 1 || selected.items[layerIndex + 1]?.locked === true} icon={ArrowUp} onClick={() => moveLayer(1)}>Move layer forward</Button>
            <Button disabled={props.disabled || props.readOnlyDraft || layer.locked || layerIndex <= 0 || selected.items[layerIndex - 1]?.locked === true} icon={ArrowDown} onClick={() => moveLayer(-1)}>Move layer backward</Button>
          </div>}
        </div>
      )}
    </Panel>
  )
}
