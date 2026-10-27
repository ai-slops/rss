import GstWebRTCAPI from "gstwebrtc-api"
import { useEffect, useRef, useState } from "react"

export type BrowserMediaState = "connecting" | "live" | "stale" | "disconnected"

type MediaPlayerProps = {
  readonly stream: string
  readonly muted: boolean
  readonly audioOnly?: boolean
  readonly onState: (state: BrowserMediaState) => void
  readonly onFrames?: (frames: number) => void
}

export function MediaPlayer({ stream, muted, audioOnly = false, onState, onFrames }: MediaPlayerProps) {
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
    const protocol = window.location.protocol === "https:" ? "wss" : "ws"
    const api = new GstWebRTCAPI({
      meta: { name: `live-production-${stream}` },
      signalingServerUrl: `${protocol}://${window.location.host}/api/v1/media/${stream}/ws`,
      reconnectionTimeout: 2_000,
      webrtcConfig: {},
    })

    const attach = (producerId: string): void => {
      if (!active || session !== null) return
      const next = api.createConsumerSession(producerId)
      if (next === null) return
      session = next
      next.addEventListener("streamsChanged", () => {
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
      })
      next.addEventListener("stateChanged", () => console.info(`WebRTC ${stream} state ${next.state}`))
      next.addEventListener("closed", () => {
        session = null
        onState("disconnected")
        if (active && retryTimer === 0) {
          retryTimer = window.setTimeout(() => {
            retryTimer = 0
            attach(producerId)
          }, 1_000)
        }
      })
      next.addEventListener("error", (event) => {
        if ("message" in event && typeof event.message === "string") console.error(`WebRTC ${stream}: ${event.message}`)
        else console.error(`WebRTC ${stream} session failed`)
        onState("disconnected")
      })
      next.connect()
    }

    api.registerConnectionListener({
      connected: () => {
        onState("connecting")
        const producer = api.getAvailableProducers()[0]
        if (producer !== undefined) attach(producer.id)
      },
      disconnected: () => onState("stale"),
    })
    api.registerPeerListener({ producerAdded: (producer) => attach(producer.id) })

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
      session?.close()
      media.pause()
      media.srcObject = null
      if (frameCallback !== 0 && media instanceof HTMLVideoElement) media.cancelVideoFrameCallback(frameCallback)
      if (retryTimer !== 0) window.clearTimeout(retryTimer)
      window.clearInterval(staleTimer)
      media.removeEventListener("playing", handlePlaying)
      api.unregisterAllConnectionListeners()
      api.unregisterAllPeerListeners()
    }
  }, [audioOnly, onFrames, onState, stream])

  useEffect(() => {
    const media = mediaRef.current
    if (media === null) return
    media.muted = muted
    if (!muted && media.srcObject !== null) void media.play().then(() => setPlaybackBlocked(false)).catch(() => setPlaybackBlocked(true))
  }, [muted])

  if (audioOnly) {
    return <audio aria-label="Private source monitor" autoPlay muted={muted} ref={(node) => { mediaRef.current = node }} />
  }
  return (
    <>
      <video aria-label={`${stream} server-rendered media`} autoPlay muted={muted} playsInline ref={(node) => { mediaRef.current = node }} />
      {playbackBlocked ? <button className="monitor__play" onClick={() => void mediaRef.current?.play()} type="button">Enable playback</button> : null}
    </>
  )
}
