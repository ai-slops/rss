declare module "@live-production/gst-channel" {
  import type GstWebRTCAPI from "gstwebrtc-api"
  export default class SignallingChannel extends EventTarget {
    constructor(url: string, meta: Readonly<Record<string, unknown>>, configuration: RTCConfiguration)
    close(): void
    createConsumerSession(producerId: string): ReturnType<GstWebRTCAPI["createConsumerSession"]> | null
  }
}
