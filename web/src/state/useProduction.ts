import { useCallback, useEffect, useRef, useState } from "react"
import { ApiError, commandId, getSnapshot, login, logout, sendCommand } from "../api/client"
import type { Command, CommandResponse, Snapshot } from "../api/schema"

export type ConnectionState = "connecting" | "connected" | "stale" | "unauthorized"

export function useProduction() {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null)
  const [connection, setConnection] = useState<ConnectionState>("connecting")
  const [error, setError] = useState<string | null>(null)
  const [connectionError, setConnectionError] = useState<string | null>(null)
  const [pending, setPending] = useState<ReadonlySet<string>>(new Set())
  const cursor = useRef<string | null>(null)

  const refresh = useCallback(async (): Promise<void> => {
    try {
      const next = await getSnapshot()
      cursor.current = next.cursor
      setSnapshot(next)
      setConnection("connected")
      setConnectionError(null)
    } catch (caught) {
      if (caught instanceof ApiError && caught.status === 401) {
        setConnection("unauthorized")
        setSnapshot(null)
        return
      }
      if (caught instanceof ApiError) {
        setConnectionError(caught.message)
      }
      setConnection("stale")
    }
  }, [])

  useEffect(() => {
    void refresh()
  }, [refresh])

  useEffect(() => {
    if (connection !== "connected" || cursor.current === null) return
    const events = new EventSource(`/api/v1/events?after=${encodeURIComponent(cursor.current)}`)
    const handleEvent = (): void => void refresh()
    for (const kind of ["production.applied", "production.rejected", "production.failed"]) events.addEventListener(kind, handleEvent)
    events.addEventListener("plugin.external", handleEvent)
    events.addEventListener("gap", () => void refresh())
    events.onerror = () => {
      setConnection("stale")
      events.close()
      void refresh()
    }
    return () => events.close()
  }, [connection, refresh])

  useEffect(() => {
    if (connection === "unauthorized" || connection === "connecting") return
    const timer = window.setInterval(() => void refresh(), 2000)
    return () => window.clearInterval(timer)
  }, [connection, refresh])

  const authenticate = useCallback(async (password: string): Promise<void> => {
    setError(null)
    setConnectionError(null)
    try {
      await login(password)
      await refresh()
    } catch (caught) {
      if (caught instanceof ApiError) {
        setError(caught.message)
        return
      }
      throw caught
    }
  }, [refresh])

  const signOut = useCallback(async (): Promise<void> => {
    await logout()
    setSnapshot(null)
    setConnection("unauthorized")
  }, [])

  const command = useCallback(async (input: Omit<Command, "id">): Promise<CommandResponse | null> => {
    if (connection !== "connected") return null
    const id = commandId()
    setPending((current) => new Set(current).add(input.kind))
    setError(null)
    try {
      const response = await sendCommand({ ...input, id })
      if (response.status === "rejected" || response.status === "failed") {
        setError(response.reason ?? "The command was not applied.")
      }
      await refresh()
      return response
    } catch (caught) {
      if (caught instanceof ApiError) {
        if (caught.status === 0) { setConnectionError(caught.message); setConnection("stale") }
        else setError(caught.message)
        if (caught.status === 401) setConnection("unauthorized")
        return null
      }
      throw caught
    } finally {
      setPending((current) => {
        const next = new Set(current)
        next.delete(input.kind)
        return next
      })
    }
  }, [connection, refresh])

  return { snapshot, connection, error: connectionError ?? error, pending, authenticate, signOut, refresh, command }
}
