import { ArrowsOutSimple, Crosshair, Trash } from "@phosphor-icons/react"
import { useEffect, useRef, useState } from "react"
import type { Scene, SceneItem } from "../api/schema"
import { Button, EmptyState, Field, Panel } from "./Primitives"

type InspectorProps = {
  readonly scene: Scene | undefined
  readonly item: SceneItem | undefined
  readonly disabled: boolean
  readonly onPatch: (item: SceneItem) => void
  readonly onRemove: () => void
  readonly canvas: { readonly width: number; readonly height: number }
  readonly aspectLocked: boolean
  readonly onAspectLockChange: (value: boolean) => void
}

function NumericField({ label, value, disabled, onChange }: { readonly label: string; readonly value: number; readonly disabled: boolean; readonly onChange: (value: number) => void }) {
  const [draft, setDraft] = useState(String(value))
  const focused = useRef(false)
  const commit = useRef(onChange)
  useEffect(() => { if (!focused.current) setDraft(String(value)) }, [value])
  return <Field label={label}><input disabled={disabled} inputMode="decimal" onFocus={() => { focused.current = true; commit.current = onChange }} onChange={(event) => setDraft(event.currentTarget.value)} onBlur={(event) => {
    focused.current = false
    const next = event.currentTarget.valueAsNumber
    if (Number.isFinite(next) && next !== value) commit.current(next)
    else setDraft(String(value))
  }} onKeyDown={(event) => { if (event.key === "Enter" && !event.nativeEvent.isComposing) event.currentTarget.blur() }} step="any" type="number" value={draft} /></Field>
}

export function Inspector({ scene, item, disabled, onPatch, onRemove, canvas, aspectLocked, onAspectLockChange }: InspectorProps) {
  if (scene === undefined || item === undefined) {
    return <Panel className="inspector" scope="DRAFT" title="Inspector"><EmptyState detail="Choose a layer to edit its rectangle, crop, fit, and opacity." title="No layer selected" /></Panel>
  }

  const updateRect = (key: keyof SceneItem["rect"], value: number): void => {
    if (!Number.isFinite(value)) return
    const ratio = item.rect.width / item.rect.height
    const linked = aspectLocked && key === "width" ? { height: value / ratio } : aspectLocked && key === "height" ? { width: value * ratio } : {}
    onPatch({ ...item, rect: { ...item.rect, ...linked, [key]: value } })
  }
  const updateCrop = (key: keyof SceneItem["crop"], value: number): void => { if (Number.isFinite(value)) onPatch({ ...item, crop: { ...item.crop, [key]: value } }) }

  return (
    <Panel className="inspector" scope="DRAFT" title="Inspector">
      <div className="inspector__identity"><span className="layer-kind">{item.content.kind}</span><div><strong>{item.name}</strong><small>{item.locked ? "Locked placement" : `Scene ${scene.revision}`}</small></div></div>
      <Field label="Layer name"><input key={item.id} defaultValue={item.name} disabled={disabled} maxLength={100} onBlur={(event) => { const name = event.currentTarget.value.trim(); if (name !== "" && name !== item.name) onPatch({ ...item, name }) }} /></Field>
      <fieldset className="field-group" disabled={disabled || item.locked}><legend>Canvas rectangle</legend><label><input checked={aspectLocked} onChange={(event) => onAspectLockChange(event.currentTarget.checked)} type="checkbox" /> Lock aspect ratio</label><div className="field-grid"><NumericField disabled={disabled} label="X" onChange={(value) => updateRect("x", value)} value={item.rect.x} /><NumericField disabled={disabled} label="Y" onChange={(value) => updateRect("y", value)} value={item.rect.y} /><NumericField disabled={disabled} label="Width" onChange={(value) => updateRect("width", value)} value={item.rect.width} /><NumericField disabled={disabled} label="Height" onChange={(value) => updateRect("height", value)} value={item.rect.height} /></div><div className="cluster"><Button icon={Crosshair} onClick={() => onPatch({ ...item, rect: { ...item.rect, x: (canvas.width - item.rect.width) / 2, y: (canvas.height - item.rect.height) / 2 } })}>Center</Button><Button icon={ArrowsOutSimple} onClick={() => onPatch({ ...item, rect: { x: 0, y: 0, width: canvas.width, height: canvas.height } })}>Fit canvas</Button><Button onClick={() => onPatch({ ...item, rect: { x: 0, y: 0, width: canvas.width, height: canvas.height }, crop: { left: 0, top: 0, right: 0, bottom: 0 }, opacity: 1, fit: "contain" })}>Reset transform</Button></div></fieldset>
      <fieldset className="field-group" disabled={disabled || item.locked}><legend>Crop</legend><div className="field-grid"><NumericField disabled={disabled} label="Left" onChange={(value) => updateCrop("left", value)} value={item.crop.left} /><NumericField disabled={disabled} label="Top" onChange={(value) => updateCrop("top", value)} value={item.crop.top} /><NumericField disabled={disabled} label="Right" onChange={(value) => updateCrop("right", value)} value={item.crop.right} /><NumericField disabled={disabled} label="Bottom" onChange={(value) => updateCrop("bottom", value)} value={item.crop.bottom} /></div></fieldset>
      <Field label="Fit mode"><select disabled={disabled || item.locked} onChange={(event) => {
        const fit = event.currentTarget.value
        if (fit === "contain" || fit === "cover" || fit === "stretch") onPatch({ ...item, fit })
      }} value={item.fit}><option value="contain">Contain</option><option value="cover">Cover</option><option value="stretch">Stretch</option></select></Field>
      <Field label={`Opacity ${Math.round(item.opacity * 100)}%`}><input disabled={disabled || item.locked} max="1" min="0" onChange={(event) => onPatch({ ...item, opacity: event.currentTarget.valueAsNumber })} step="0.01" type="range" value={item.opacity} /></Field>
      <Button disabled={disabled || item.locked} icon={Trash} onClick={onRemove} tone="danger">Remove placement</Button>
    </Panel>
  )
}
