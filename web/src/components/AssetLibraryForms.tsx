import { Plus } from "@phosphor-icons/react"
import { useState } from "react"
import { z } from "zod"
import type { SceneItem } from "../api/schema"
import { Button, Field } from "./Primitives"

export type StaticContent = Exclude<SceneItem["content"], { readonly kind: "track" }>

export type StaticPlacementFormProps = {
  readonly disabled: boolean
  readonly onAdd: (name: string, content: StaticContent) => Promise<void>
}

const layerNameSchema = z.string().trim().min(1, "Enter a layer name.").max(100, "Use at most 100 characters.")
const colorSchema = z.string().regex(/^#[0-9a-f]{6}$/iu, "Choose a six-digit color.")
const solidSchema = z.object({ name: layerNameSchema, color: colorSchema })
const textSchema = z.object({
  name: layerNameSchema,
  text: z.string().min(1, "Enter text content.").refine((value) => new TextEncoder().encode(value).byteLength <= 4_096, "Text is limited to 4096 bytes."),
  color: colorSchema,
  fontSize: z.coerce.number().int().min(8, "Font size must be 8–256 px.").max(256, "Font size must be 8–256 px."),
})

function assertNever(value: never): never {
  throw new TypeError(`Unhandled static placement kind: ${String(value)}`)
}

export function StaticPlacementForms({ disabled, onAdd }: StaticPlacementFormProps) {
  const [solidName, setSolidName] = useState("Background")
  const [solidColor, setSolidColor] = useState("#10161b")
  const [textName, setTextName] = useState("Title")
  const [text, setText] = useState("Starting soon")
  const [textColor, setTextColor] = useState("#ffffff")
  const [fontSize, setFontSize] = useState("48")
  const [pending, setPending] = useState<"solid" | "text" | null>(null)
  const [error, setError] = useState<string | null>(null)

  const add = (kind: "solid" | "text"): void => {
    let operation: Promise<void>
    switch (kind) {
      case "solid": {
        const parsed = solidSchema.safeParse({ name: solidName, color: solidColor })
        if (!parsed.success) {
          setError(parsed.error.issues[0]?.message ?? "Review the solid placement values.")
          return
        }
        operation = onAdd(parsed.data.name, { kind: "solid", color: parsed.data.color })
        break
      }
      case "text": {
        const parsed = textSchema.safeParse({ name: textName, text, color: textColor, fontSize })
        if (!parsed.success) {
          setError(parsed.error.issues[0]?.message ?? "Review the text placement values.")
          return
        }
        operation = onAdd(parsed.data.name, { kind: "text", text: parsed.data.text, color: parsed.data.color, fontSize: parsed.data.fontSize })
        break
      }
      default:
        assertNever(kind)
    }
    setError(null)
    setPending(kind)
    void operation.catch((caught: unknown) => {
      if (caught instanceof Error) setError(caught.message)
      else throw caught
    }).finally(() => setPending(null))
  }

  return (
    <div className="production-setup__static-grid">
      <form className="production-setup__card" onSubmit={(event) => { event.preventDefault(); add("solid") }}>
        <h4>Solid color</h4>
        <Field label="Layer name"><input disabled={disabled || pending !== null} maxLength={100} onChange={(event) => setSolidName(event.currentTarget.value)} required value={solidName} /></Field>
        <Field label="Color"><input disabled={disabled || pending !== null} onChange={(event) => setSolidColor(event.currentTarget.value)} type="color" value={solidColor} /></Field>
        <Button disabled={disabled || pending !== null || solidName.trim().length === 0} icon={Plus} pending={pending === "solid"} tone="preview" type="submit">Add solid to Preview</Button>
      </form>
      <form className="production-setup__card" onSubmit={(event) => { event.preventDefault(); add("text") }}>
        <h4>Text layer</h4>
        <Field label="Layer name"><input disabled={disabled || pending !== null} maxLength={100} onChange={(event) => setTextName(event.currentTarget.value)} required value={textName} /></Field>
        <Field label="Text"><textarea disabled={disabled || pending !== null} maxLength={4_096} onChange={(event) => setText(event.currentTarget.value)} required value={text} /></Field>
        <div className="production-setup__form-grid"><Field label="Color"><input disabled={disabled || pending !== null} onChange={(event) => setTextColor(event.currentTarget.value)} type="color" value={textColor} /></Field><Field label="Font size"><div className="unit-input"><input disabled={disabled || pending !== null} max="256" min="8" onChange={(event) => setFontSize(event.currentTarget.value)} type="number" value={fontSize} /><span>px</span></div></Field></div>
        <Button disabled={disabled || pending !== null || textName.trim().length === 0 || text.length === 0} icon={Plus} pending={pending === "text"} tone="preview" type="submit">Add text to Preview</Button>
      </form>
      {error === null ? null : <p className="production-setup__error" role="alert">{error}</p>}
    </div>
  )
}
