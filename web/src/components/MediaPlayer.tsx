import SignallingChannel from "@live-production/gst-channel"
import type GstWebRTCAPI from "gstwebrtc-api"
import { useEffect, useRef, useState } from "react"
import { z } from "zod"

const producerEventSchema = z.object({ role: z.string(), peer: z.object({ id: z.string().min(1) }) })

export type BrowserMediaState = "connecting" | "live" | "stale" | "disconnected"

type MediaPlayerProps = {
  readonly stream: string
  readonly muted: boolean
  readonly volume?: number
  readonly audioOnly?: boolean
  readonly onState: (state: BrowserMediaState) => void
  readonly onFrames?: (frames: number) => void
}

export function MediaPlayer({ stream, muted, volume = 1, audioOnly = false, onState, onFrames }: MediaPlayerProps) {
  const mediaRef = useRef<HTMLVideoElement | HTMLAudioElement>(null)
  const [playbackBlocked, setPlaybackBlocked] = useState(false)

  useEffect(() => {
    const media = mediaRef.current
    if (media === null) return
    let session: ReturnType<GstWebRTCAPI["createConsumerSession"]> | null = null
    let frameCallback = 0
    let frames = 0
    let observedFrames = 0
    let active = true
    let retryTimer = 0
    let channel: SignallingChannel | null = null
    let detachListeners: (() => void) | null = null
    let detachSession: (() => void) | null = null
    const protocol = window.location.protocol === "https:" ? "wss" : "ws"

    const stopChannel = (): void => {
      detachListeners?.(); detachListeners = null
      detachSession?.(); detachSession = null
      session?.close(); session = null
      channel?.close(); channel = null
    }

    const retry = (): void => {
      stopChannel()
      if (active && retryTimer === 0) retryTimer = window.setTimeout(() => { retryTimer = 0; connect() }, 2000)
    }

    const attach = (producerId: string): void => {
      if (!active || session !== null) return
      const next = channel?.createConsumerSession(producerId)
      if (next === null || next === undefined) return
      session = next
      const changed = (): void => {
        const remote = next.streams[0]
        if (remote === undefined || !active) return
        media.srcObject = remote
        void media.play().then(() => {
          setPlaybackBlocked(false)
        }).catch((error: unknown) => {
          if (error instanceof DOMException && error.name === "NotAllowedError") {
            setPlaybackBlocked(true)
            return
          }
          onState("disconnected")
        })
      }
      const closed = (): void => {
        onState("disconnected")
        retry()
      }
      next.addEventListener("streamsChanged", changed)
      next.addEventListener("closed", closed)
      next.addEventListener("error", closed)
      detachSession = () => { next.removeEventListener("streamsChanged", changed); next.removeEventListener("closed", closed); next.removeEventListener("error", closed) }
      next.connect()
    }

    const connect = (): void => {
      if (!active) return
      channel = new SignallingChannel(`${protocol}://${window.location.host}/api/v1/media/${stream}/ws`, { name: `live-production-${stream}` }, { iceServers: [] })
      const producerAdded = (event: Event): void => {
        if (!(event instanceof CustomEvent)) return
        const detail = producerEventSchema.safeParse(event.detail)
        if (detail.success && detail.data.role === "producer") attach(detail.data.peer.id)
      },
      ready = (): void => onState("connecting"),
      ended = (): void => { onState("disconnected"); retry() }
      channel.addEventListener("peerAdded", producerAdded)
      channel.addEventListener("ready", ready)
      channel.addEventListener("closed", ended)
      channel.addEventListener("error", ended)
      const ownedChannel = channel
      detachListeners = () => { ownedChannel.removeEventListener("peerAdded", producerAdded); ownedChannel.removeEventListener("ready", ready); ownedChannel.removeEventListener("closed", ended); ownedChannel.removeEventListener("error", ended) }
    }
    connect()

    if (!audioOnly && media instanceof HTMLVideoElement && "requestVideoFrameCallback" in media) {
      const countFrame: VideoFrameRequestCallback = () => {
        if (!active) return
        frames += 1
        onFrames?.(frames)
        onState("live")
        frameCallback = media.requestVideoFrameCallback(countFrame)
      }
      frameCallback = media.requestVideoFrameCallback(countFrame)
    }
    const staleTimer = window.setInterval(() => {
      if (frames > 0 && frames === observedFrames) onState("stale")
      observedFrames = frames
    }, 2_000)
    const handlePlaying = (): void => {
      if (audioOnly) onState("live")
    }
    media.addEventListener("playing", handlePlaying)

    return () => {
      active = false
      stopChannel()
      media.pause()
      media.srcObject = null
      if (frameCallback !== 0 && media instanceof HTMLVideoElement) media.cancelVideoFrameCallback(frameCallback)
      if (retryTimer !== 0) window.clearTimeout(retryTimer)
      window.clearInterval(staleTimer)
      media.removeEventListener("playing", handlePlaying)
    }
  }, [audioOnly, onFrames, onState, stream])

  useEffect(() => {
    const media = mediaRef.current
    if (media === null) return
    media.muted = muted
    media.volume = volume
    if (!muted && media.srcObject !== null) void media.play().then(() => setPlaybackBlocked(false)).catch(() => setPlaybackBlocked(true))
  }, [muted, volume])

  return (
    <>
      {audioOnly ? <audio aria-label="Private source monitor" autoPlay muted={muted} ref={(node) => { mediaRef.current = node }} /> : <video aria-label={`${stream} server-rendered media`} autoPlay muted={muted} playsInline ref={(node) => { mediaRef.current = node }} />}
      {playbackBlocked ? <button className={audioOnly ? "button button--info" : "monitor__play"} onClick={() => { const media = mediaRef.current; if (media !== null) void media.play().then(() => setPlaybackBlocked(false)).catch(() => setPlaybackBlocked(true)) }} type="button">Enable {audioOnly ? "private monitor " : ""}playback</button> : null}
    </>
  )
}
