import { ArrowsOutSimple, Crosshair, Trash } from "@phosphor-icons/react"
import type { Scene, SceneItem } from "../api/schema"
import { Button, EmptyState, Field, Panel } from "./Primitives"

type InspectorProps = {
  readonly scene: Scene | undefined
  readonly item: SceneItem | undefined
  readonly disabled: boolean
  readonly onPatch: (item: SceneItem) => void
  readonly onRemove: () => void
}

function NumericField({ label, value, disabled, onChange }: { readonly label: string; readonly value: number; readonly disabled: boolean; readonly onChange: (value: number) => void }) {
  return <Field label={label}><input disabled={disabled} inputMode="decimal" onChange={(event) => onChange(event.currentTarget.valueAsNumber)} type="number" value={value} /></Field>
}

export function Inspector({ scene, item, disabled, onPatch, onRemove }: InspectorProps) {
  if (scene === undefined || item === undefined) {
    return <Panel className="inspector" scope="DRAFT" title="Inspector"><EmptyState detail="Choose a layer to edit its rectangle, crop, fit, and opacity." title="No layer selected" /></Panel>
  }

  const updateRect = (key: keyof SceneItem["rect"], value: number): void => onPatch({ ...item, rect: { ...item.rect, [key]: value } })
  const updateCrop = (key: keyof SceneItem["crop"], value: number): void => onPatch({ ...item, crop: { ...item.crop, [key]: value } })

  return (
    <Panel className="inspector" scope="DRAFT" title="Inspector">
      <div className="inspector__identity"><span className="layer-kind">{item.content.kind}</span><div><strong>{item.name}</strong><small>{item.locked ? "Locked placement" : `Scene ${scene.revision}`}</small></div></div>
      <fieldset className="field-group" disabled={disabled || item.locked}><legend>Canvas rectangle</legend><div className="field-grid"><NumericField disabled={disabled} label="X" onChange={(value) => updateRect("x", value)} value={item.rect.x} /><NumericField disabled={disabled} label="Y" onChange={(value) => updateRect("y", value)} value={item.rect.y} /><NumericField disabled={disabled} label="Width" onChange={(value) => updateRect("width", value)} value={item.rect.width} /><NumericField disabled={disabled} label="Height" onChange={(value) => updateRect("height", value)} value={item.rect.height} /></div><div className="cluster"><Button icon={Crosshair} onClick={() => onPatch({ ...item, rect: { ...item.rect, x: 0, y: 0 } })}>Center</Button><Button icon={ArrowsOutSimple} onClick={() => onPatch({ ...item, rect: { x: 0, y: 0, width: 1920, height: 1080 } })}>Fit canvas</Button></div></fieldset>
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
