import { useState } from "react"
import { z } from "zod"
import type { Snapshot } from "../api/schema"
import { Button, Field } from "./Primitives"

type Source = Snapshot["project"]["sources"][number]
type SourceAdapter = Source["adapter"]

export type NewSourcePayload = { readonly name: string; readonly adapter: SourceAdapter }
export type SourceFormDraft =
  | { readonly kind: "srt"; readonly name: string; readonly port: string; readonly bufferMs: string }
  | { readonly kind: "synthetic"; readonly name: string; readonly pattern: string; readonly frequencyHz: string }

export type SourceFormResult =
  | { readonly kind: "valid"; readonly payload: NewSourcePayload }
  | { readonly kind: "invalid"; readonly message: string }

const nameSchema = z.string().trim().min(1, "Enter a source name.").max(100, "Use at most 100 characters.").refine(
  (value) => Array.from(value).every((character) => {
    const codePoint = character.codePointAt(0)
    return codePoint !== undefined && codePoint >= 32 && codePoint !== 127
  }),
  "Remove control characters from the name.",
)
const formSchema = z.discriminatedUnion("kind", [
  z.object({
    kind: z.literal("srt"),
    name: nameSchema,
    port: z.coerce.number().int().min(1, "Port must be between 1 and 65535.").max(65_535, "Port must be between 1 and 65535."),
    bufferMs: z.coerce.number().int().min(0, "Buffer must be non-negative.").max(60_000, "Buffer must be at most 60000 ms."),
  }),
  z.object({
    kind: z.literal("synthetic"),
    name: nameSchema,
    pattern: z.enum(["ball", "black", "smpte", "snow"], "Choose a supported pattern."),
    frequencyHz: z.coerce.number().int().min(1, "Frequency must be positive.").max(20_000, "Frequency must be at most 20000 Hz."),
  }),
])

export function parseSourceForm(draft: SourceFormDraft): SourceFormResult {
  const parsed = formSchema.safeParse(draft)
  if (!parsed.success) return { kind: "invalid", message: parsed.error.issues[0]?.message ?? "Review the source values." }
  switch (parsed.data.kind) {
    case "srt":
      return { kind: "valid", payload: { name: parsed.data.name, adapter: { kind: "srt", bindAddress: "0.0.0.0", port: parsed.data.port, bufferMs: parsed.data.bufferMs } } }
    case "synthetic":
      return { kind: "valid", payload: { name: parsed.data.name, adapter: { kind: "synthetic", pattern: parsed.data.pattern, frequencyHz: parsed.data.frequencyHz } } }
  }
}

type SourceEditorProps = {
  readonly source?: Source
  readonly disabled: boolean
  readonly submitLabel: string
  readonly onSubmit: (payload: NewSourcePayload) => Promise<void>
}

function SourceEditor({ source, disabled, submitLabel, onSubmit }: SourceEditorProps) {
  const [kind, setKind] = useState<SourceAdapter["kind"]>(source?.adapter.kind ?? "srt")
  const [name, setName] = useState(source?.name ?? "")
  const [port, setPort] = useState(source?.adapter.kind === "srt" ? String(source.adapter.port) : "9000")
  const [bufferMs, setBufferMs] = useState(source?.adapter.kind === "srt" ? String(source.adapter.bufferMs) : "120")
  const [pattern, setPattern] = useState(source?.adapter.kind === "synthetic" ? source.adapter.pattern : "smpte")
  const [frequencyHz, setFrequencyHz] = useState(source?.adapter.kind === "synthetic" ? String(source.adapter.frequencyHz) : "440")
  const [error, setError] = useState<string | null>(null)
  const [pending, setPending] = useState(false)

  return (
    <form className="source-editor" onSubmit={(event) => {
      event.preventDefault()
      const draft: SourceFormDraft = kind === "srt" ? { kind, name, port, bufferMs } : { kind, name, pattern, frequencyHz }
      const parsed = parseSourceForm(draft)
      if (parsed.kind === "invalid") {
        setError(parsed.message)
        return
      }
      setError(null)
      setPending(true)
      void onSubmit(parsed.payload).catch((caught: unknown) => {
        if (caught instanceof Error) setError(caught.message)
        else throw caught
      }).finally(() => setPending(false))
    }}>
      <div className="production-setup__form-grid">
        <Field label="Source name"><input disabled={disabled || pending} maxLength={100} onChange={(event) => setName(event.currentTarget.value)} required value={name} /></Field>
        <Field label="Input type"><select disabled={disabled || pending} onChange={(event) => setKind(event.currentTarget.value === "synthetic" ? "synthetic" : "srt")} value={kind}><option value="srt">SRT listener</option><option value="synthetic">Synthetic fixture</option></select></Field>
        {kind === "srt" ? (
          <><Field label="Listen port"><input disabled={disabled || pending} inputMode="numeric" max="65535" min="1" onChange={(event) => setPort(event.currentTarget.value)} required type="number" value={port} /></Field><Field label="Transport buffer"><div className="unit-input"><input disabled={disabled || pending} inputMode="numeric" max="60000" min="0" onChange={(event) => setBufferMs(event.currentTarget.value)} required type="number" value={bufferMs} /><span>ms</span></div></Field></>
        ) : (
          <><Field label="Pattern"><select disabled={disabled || pending} onChange={(event) => setPattern(event.currentTarget.value)} value={pattern}><option value="smpte">SMPTE bars</option><option value="ball">Moving ball</option><option value="black">Black</option><option value="snow">Snow</option></select></Field><Field label="Audio tone"><div className="unit-input"><input disabled={disabled || pending} inputMode="numeric" max="20000" min="1" onChange={(event) => setFrequencyHz(event.currentTarget.value)} required type="number" value={frequencyHz} /><span>Hz</span></div></Field></>
        )}
      </div>
      {error === null ? null : <p className="production-setup__error" role="alert">{error}</p>}
      <Button disabled={disabled || pending} pending={pending} tone={source === undefined ? "preview" : "warning"} type="submit">{submitLabel}</Button>
    </form>
  )
}

export function CreateSourceForm(props: Omit<SourceEditorProps, "source" | "submitLabel">) {
  return <SourceEditor {...props} submitLabel="Create input" />
}

export function ConfigureSourceForm(props: Omit<SourceEditorProps, "submitLabel"> & { readonly source: Source }) {
  return <SourceEditor {...props} submitLabel="Apply shared configuration" />
}
