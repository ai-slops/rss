import { DownloadSimple, Flag, Package, Play, Stop } from "@phosphor-icons/react"
import { useState } from "react"
import { z } from "zod"
import type { Snapshot } from "../api/schema"
import { Button, EmptyState, Field, StatusChip } from "./Primitives"
import type { OperatorCommand, OperatorCommandReceipt } from "./ProductionSetup"

const EDITOR_PLUGIN_ID = "editor-session"
const EVENT_PLUGIN_ID = "external-event-panel"
const EDITOR_COMMANDS = ["session.start", "marker.add", "session.export", "session.stop"] as const
const EVENT_COMMANDS = ["event.record"] as const
const pluginTextSchema = z.string().trim().min(1).max(4_096)

type PluginPayloadInput = { readonly pluginId: string; readonly command: string; readonly text: string }

export function supportsPluginCommand(pluginId: string, command: string): boolean {
  if (pluginId === EDITOR_PLUGIN_ID) return EDITOR_COMMANDS.some((candidate) => candidate === command)
  if (pluginId === EVENT_PLUGIN_ID) return EVENT_COMMANDS.some((candidate) => candidate === command)
  return false
}

export function buildPluginPayload(input: PluginPayloadInput): Readonly<Record<string, unknown>> | null {
  if (!supportsPluginCommand(input.pluginId, input.command)) return null
  if (input.command === "session.export" || input.command === "session.stop") return { command: input.command }
  const text = pluginTextSchema.safeParse(input.text)
  return text.success ? { command: input.command, text: text.data } : null
}

function downloadPluginResult(value: unknown, sessionId: string): void {
  const safeName = sessionId.replace(/[^a-z0-9_-]+/giu, "-").replace(/^-+|-+$/gu, "") || "editor-session"
  const url = URL.createObjectURL(new Blob([JSON.stringify(value, null, 2)], { type: "application/json" }))
  const link = document.createElement("a")
  link.href = url
  link.download = `${safeName}.json`
  link.click()
  URL.revokeObjectURL(url)
}

function receiptError(receipt: OperatorCommandReceipt | null): string | null {
  if (receipt === null) return "The plugin command did not complete."
  if (receipt.status === "rejected" || receipt.status === "failed") return receipt.reason ?? "The plugin command was rejected."
  return null
}

export type PluginLibraryProps = { readonly snapshot: Snapshot; readonly disabled: boolean; readonly onCommand: OperatorCommand }

export function PluginLibrary({ snapshot, disabled, onCommand }: PluginLibraryProps) {
  const [sessionId, setSessionId] = useState("")
  const [marker, setMarker] = useState("")
  const [eventText, setEventText] = useState("")
  const [pending, setPending] = useState<string | null>(null)
  const [message, setMessage] = useState<string | null>(null)
  const editor = snapshot.plugins.find((plugin) => plugin.id === EDITOR_PLUGIN_ID)
  const eventPanel = snapshot.plugins.find((plugin) => plugin.id === EVENT_PLUGIN_ID)
  const otherPlugins = snapshot.plugins.filter((plugin) => plugin.id !== EDITOR_PLUGIN_ID && plugin.id !== EVENT_PLUGIN_ID)
  const editorHasSession = editor?.status.startsWith("Session ") === true

  const run = async (input: PluginPayloadInput): Promise<OperatorCommandReceipt> => {
    const payload = buildPluginPayload(input)
    if (payload === null) throw new TypeError("Enter the required text for this registered plugin command.")
    setPending(input.command)
    setMessage(null)
    try {
      const receipt = await onCommand("plugin.command", input.pluginId, payload)
      const failure = receiptError(receipt)
      if (failure !== null || receipt === null) throw new TypeError(failure ?? "The plugin command did not complete.")
      setMessage(`${input.command} applied.`)
      return receipt
    } finally {
      setPending(null)
    }
  }

  return (
    <div className="production-setup__stack">
      {snapshot.plugins.length === 0 ? <EmptyState detail="Only trusted server-registered extensions appear here." title="No extensions registered" /> : null}
      <section className="production-setup__section"><div className="production-setup__section-header"><div><h3>Editor session</h3><p>Journals production observations without starting or stopping outputs.</p></div><StatusChip tone={editor?.available === true ? editor.enabled ? "healthy" : "neutral" : "danger"}>{editor?.available === true ? editor.status : "UNAVAILABLE"}</StatusChip></div>
        <div className="production-setup__form-grid"><Field label="Session ID"><input disabled={disabled || editor?.available !== true || pending !== null} maxLength={100} onChange={(event) => setSessionId(event.currentTarget.value)} value={sessionId} /></Field><Button disabled={disabled || editor?.available !== true || pending !== null || sessionId.trim().length === 0} icon={Play} onClick={() => void run({ pluginId: EDITOR_PLUGIN_ID, command: "session.start", text: sessionId }).catch((caught: unknown) => { if (caught instanceof Error) setMessage(caught.message); else throw caught })} pending={pending === "session.start"} tone="preview">Start session</Button></div>
        <div className="production-setup__form-grid"><Field label="Editorial marker"><input disabled={disabled || editor?.enabled !== true || pending !== null} maxLength={100} onChange={(event) => setMarker(event.currentTarget.value)} value={marker} /></Field><Button disabled={disabled || editor?.enabled !== true || pending !== null || marker.trim().length === 0} icon={Flag} onClick={() => void run({ pluginId: EDITOR_PLUGIN_ID, command: "marker.add", text: marker }).catch((caught: unknown) => { if (caught instanceof Error) setMessage(caught.message); else throw caught })}>Add marker</Button></div>
        <div className="production-setup__actions"><Button disabled={disabled || editor?.available !== true || !editorHasSession || pending !== null} icon={DownloadSimple} onClick={() => void run({ pluginId: EDITOR_PLUGIN_ID, command: "session.export", text: "" }).then((receipt) => { if (receipt.pluginResult === undefined) throw new TypeError("The command bridge did not preserve pluginResult; export was not downloaded."); downloadPluginResult(receipt.pluginResult, sessionId) }).catch((caught: unknown) => { if (caught instanceof Error) setMessage(caught.message); else throw caught })} pending={pending === "session.export"}>Export private JSON</Button><Button disabled={disabled || editor?.enabled !== true || pending !== null} icon={Stop} onClick={() => void run({ pluginId: EDITOR_PLUGIN_ID, command: "session.stop", text: "" }).catch((caught: unknown) => { if (caught instanceof Error) setMessage(caught.message); else throw caught })} pending={pending === "session.stop"} tone="danger">Stop session</Button></div>
      </section>
      <section className="production-setup__section"><div className="production-setup__section-header"><div><h3>External event panel</h3><p>Records one namespaced external event without copying domain history into production state.</p></div><StatusChip tone={eventPanel?.available === true ? "healthy" : "danger"}>{eventPanel?.available === true ? eventPanel.status : "UNAVAILABLE"}</StatusChip></div><div className="production-setup__form-grid"><Field label="Event text"><input disabled={disabled || eventPanel?.available !== true || pending !== null} maxLength={100} onChange={(event) => setEventText(event.currentTarget.value)} value={eventText} /></Field><Button disabled={disabled || eventPanel?.available !== true || pending !== null || eventText.trim().length === 0} icon={Package} onClick={() => void run({ pluginId: EVENT_PLUGIN_ID, command: "event.record", text: eventText }).catch((caught: unknown) => { if (caught instanceof Error) setMessage(caught.message); else throw caught })} pending={pending === "event.record"}>Record event</Button></div></section>
      {otherPlugins.map((plugin) => <section className="production-setup__card" key={plugin.id}><div className="production-setup__card-header"><Package aria-hidden="true" /><div className="row-copy"><strong>{plugin.name}</strong><small>{plugin.status}</small></div><StatusChip tone="danger">UNSUPPORTED</StatusChip></div><p className="production-setup__hint">This client has no registered command contract for {plugin.id}; controls are intentionally unavailable.</p></section>)}
      {message === null ? null : <p aria-live="polite" className="production-setup__message">{message}</p>}
    </div>
  )
}
