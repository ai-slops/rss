# ADR 003: own the monitor signalling connection lifetime

The pinned `gstwebrtc-api` 3.0.0 high-level API does not expose a connection-disposal method. Removing its listeners and closing a consumer session does not close its WebSocket, and its automatic reconnection can outlive a React monitor. That can consume the bounded viewer budget after ordinary source-Listen switching.

Use the same package's unmodified `ComChannel` module through one version-pinned Vite alias. It provides a public `close()` operation and the same consumer-session implementation. React now explicitly owns channel/session listeners, reconnection timers and transport closure. No browser-native WebRTC prototypes are patched, no custom media backend is introduced, and the third-party source remains in its original package with its MPL licence.

The narrow ambient type declaration describes only the methods the application consumes. Its default-export spelling mirrors that external module contract, so the style-only `noDefaultExport` rule is excepted for this one declaration; strict types, no-`any`, unused checks and all runtime-source export rules remain enabled.

The relay additionally checks media origins and selected audio-track identity, revokes signalling when the HTTP session expires, limits receive sessions per WebSocket, and releases source-monitor pipelines when their last viewer leaves. Production/recording lifetime remains independent of browser transport lifetime.
