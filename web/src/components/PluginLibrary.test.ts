import { describe, expect, it } from "vitest"
import { buildPluginPayload, supportsPluginCommand } from "./PluginLibrary"

describe("registered plugin command contracts", () => {
  it("routes only the registered editor and external-event commands", () => {
    expect(supportsPluginCommand("editor-session", "session.export")).toBe(true)
    expect(supportsPluginCommand("external-event-panel", "event.record")).toBe(true)
    expect(supportsPluginCommand("unknown", "enable")).toBe(false)
  })

  it("omits text for commands whose contract omits it", () => {
    expect(buildPluginPayload({ pluginId: "editor-session", command: "session.export", text: "ignored" })).toEqual({ command: "session.export" })
  })

  it("includes trimmed text for commands that require it", () => {
    expect(buildPluginPayload({ pluginId: "editor-session", command: "marker.add", text: "  Goal  " })).toEqual({ command: "marker.add", text: "Goal" })
    expect(buildPluginPayload({ pluginId: "external-event-panel", command: "event.record", text: "  Donation  " })).toEqual({ command: "event.record", text: "Donation" })
  })
})
