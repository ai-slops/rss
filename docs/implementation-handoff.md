# Web-Controlled Live Production System
## Implementation Handoff

**Date:** 2026-10-27  
**Status:** Implementation baseline, with explicit engineering validation gates.  
**Audience:** An implementation agent working in a new or existing repository.

This document is self-contained. Requirements stated here replace earlier speculative designs. Product behavior is specified; implementation defaults may change when repository constraints or measured results justify it. Record such changes in a short architecture decision record rather than reopening the entire design.

## 1. Product and boundaries

Build a live production application running headlessly on Ubuntu Server and operated from a laptop browser. It receives video/audio, supports scene composition and audio mixing, compensates source timing differences, records media, and sends selected results to streaming destinations. Closing the browser must not stop production.

An external computer may have only OBS Studio and a small plugin installed. It must not require Tailscale, a companion desktop application, or platform credentials. Remote capture management is optional; an ordinary authenticated network stream must work without device enrollment. Local and remote sources use the same production model.

The system is a complete broadcasting product, not merely a capture service or an editor backend. Capture-appliance management, platform APIs, chat/donations/subscriptions/viewer counts, game/application events, and editor-session export are extensions. Built-in network input, recording, and network output implementations may themselves be bundled plugins.

Editor integration supports deliberate, high-quality post-production. It does not require live import, immediate rough cuts, automated highlight generation, local capture masters, or deterministic rerendering. Server-received source recordings may be edit masters. Files copied later from a capture device may optionally replace them. Semantic reconstruction and a preserved Program reference are sufficient.

## 2. Decisions and rationale

These are project choices, not claims that every broadcasting product should behave this way.

| Decision | Baseline | Reason |
|---|---|---|
| Editing versus broadcasting | Draft scene revisions are separate from the applied Program snapshot. | Safe layout editing must not modify the current broadcast. |
| Scope of that isolation | Layout/content parameters are isolated; live input devices, source processing, timing, and the audio mixer are shared unless explicitly duplicated. | A scene snapshot cannot duplicate a physical camera, upstream feed, or live mixer. |
| After Take | Keep the current Preview selection. Provide **Cue previous Program** separately. | Better for iterative layout editing and avoids unexpectedly editing the old scene. Traditional A/B swap is useful but is not the only valid workflow. [R1, R2] |
| Audio | Independent live mixer; audio-follow-video is off. | Prevent scene visibility from unexpectedly muting microphones or duplicating audio routes. Existing mixers distinguish these operations. [R3] |
| Monitoring | Private browser monitoring, initially muted; source Listen never changes Program. | Monitoring is not a broadcast mute/solo operation. [R3, R12] |
| Timing | Per-track correction, explicit common holdback, bounded buffering. | Manual correction, transport buffering, and playout latency solve different problems. [R4] |
| Applying timing | Edit values locally, then apply explicitly; no automatic global latency changes while live. | Avoid invisible disruption to every active source. |
| Output control | Recording and each destination have independent desired/actual states. | One output must not control the lifetime of all others. |
| Public-stream safety | Starting destination transmission is potentially public. | YouTube can start a broadcast automatically when its bound feed starts. “Connect encoder” is not universally a private operation. [R10] |
| Preview truth | Server-rendered image plus browser interaction overlay; distinguish saved, rendered, and displayed state. | Browser delivery lags commands; receiving an acknowledgment does not prove a corresponding image is visible. [R11] |
| Plugin scope | Trusted, registered modules initially. | Extension boundaries are needed now; marketplaces, hot reload, and a universal plugin ABI are not. |
| Session ownership | Editor sessions belong to the editor plugin, not the media-engine lifecycle. | Recording/export boundaries must not dictate how the broadcaster runs. |

No additional product decision is required before implementation. Hardware-dependent claims remain subject to the gates in Section 16.

## 3. First release and engineering defaults

The first usable release includes a scene editor, separate Preview and Program, at least two independent video inputs, at least two independently routed audio tracks, source timing controls, browser monitoring, clean-source and Program recording, and at least one real RTMPS destination. Data structures and output lifecycle must permit multiple destinations; test output independence using a second controlled destination.

Ship cut transitions first. Include basic static image, text, and solid-color content so starting/break/standby scenes do not require a browser plugin. Defer advanced transitions, nested scenes, arbitrary shaders, browser-source execution, automated file playback, and sophisticated audio effects. Do not show nonfunctional controls as if these features work.

Respect an existing repository's language and framework. For a greenfield implementation, the default is a Rust server using the official GStreamer bindings, a TypeScript/React UI, SQLite, and ordinary local files. These are engineering defaults, not plugin contracts. A modular daemon is sufficient; native-code crash isolation is not claimed for in-process modules. Keep media callbacks and general application work on separate execution paths.

Use one GStreamer backend first. It supplies composition, mixing, timing, and recording primitives, while its Rust bindings are maintained within the project. [R4, R5, R6, R8, R15] Start with CPU/synthetic test paths; add the target machine's supported hardware encoding after measurement. Do not assume an NVIDIA GPU or commit to a particular hardware API.

Libobs remains a legitimate alternative, not something disqualified by the lack of an OBS GUI: it is a library providing media subsystems and plugin infrastructure. [R14] Reconsider it only if an initial gate demonstrates a material GStreamer problem. Do not implement two interchangeable backends or a custom compositor preemptively.

Other existing systems were checked rather than assuming a new engine is necessary. Smelter already offers declarative video/audio composition through React or HTTP, including an RTMP input. [R20, R21] However, its documented input-update route covers MP4 pause/seek; this research did not establish a ready-made path for the required independently adjustable live-track timing and original-source archive taps. That is a verification gap, not proof that Smelter cannot be extended. It is a bounded alternative when it demonstrably reduces implementation work, not an unconditional replacement or another backend to build in parallel. Review current upstream distribution terms before adopting it. Sofie's documented focus is web-based production automation and device control, making it a useful control-surface reference rather than a demonstrated substitute for this media engine. [R22]

Initial transport target: OBS-compatible SRT carrying a tested H.264/AAC MPEG-TS profile, and H.264/AAC over RTMPS for public output. Add RTMP ingress when the actual sender workflow requires it. Protocol details belong to adapters, not scene definitions. OBS documents SRT publishing, but arbitrary independent source output and audio separation still need validation against the chosen sender/plugin. [R16]

Choose and lock dependency versions during setup. This specification deliberately does not claim any pinned version or hardware throughput has already been tested.

## 4. Domain and state model

| Object | Meaning |
|---|---|
| Project | Persisted sources, scene drafts, mix configuration, output definitions, assets, and namespaced plugin settings. |
| Source | Stable logical input; its transport connection can be replaced or reconnected. |
| Track | A stable selectable video or audio stream within a Source. |
| Scene | A reusable visual layout with a saved draft revision. |
| SceneItem | A placement with its own ID, referencing a track or revisioned static content. |
| AudioMix | Explicit audio routes and their live parameters. |
| Program | Applied visual snapshot and the live mix used to produce output. Implement one active Program initially. |
| Output | Independently controlled consumer of a source tap or Program, such as a recorder or network publisher. |

Do not create separate user-managed rendition, recording-session, capture-node, and distributed-clock hierarchies before they are needed. Internal encoder sharing and connection generations are implementation details.

Use opaque stable IDs, rational frame rates/time bases, and explicit units. JSON media timestamps and sample positions must not lose precision; decimal strings are acceptable. Names and file paths are not identity.

Separate persisted configuration, requested runtime state, and observed runtime state. UI-only selection, panel sizes, monitor volume, and temporary pointer gestures are not broadcast configuration.

A Program snapshot pins layout, static content parameters, and asset versions. It retains live track references rather than copying a stream. Changing a shared source still affects every current consumer. Source-level controls must make that scope visible.

## 5. Web interface and interaction rules

### 5.1 Information architecture

Use one main **Production** workspace with resizable, fixed-purpose panels. Do not build a general docking/window-manager framework.

```text
Header: project | control connection | Program health | recordings | destinations

Scenes + Layers | Preview / scene-edit surface | Program (read-only) | Inspector

Audio mixer and local monitoring | Inputs/Sync | Output/Recording controls
```

Inputs, output setup, recordings, assets, and plugin settings may use drawers or secondary pages. Keep a compact Program/status area visible during setup. On a small laptop, allow focus on either video pane without confusing their labels. Full mobile scene editing is not a first-release requirement.

A new project starts with a neutral standby scene, no network output running, and no unapproved audio route. Setup should lead the operator through adding an input, inspecting its tracks, adding video to a scene, explicitly routing audio, checking Preview, taking the scene, and independently starting recording or transmission.

Use text/state icons as well as color. Show whether a control affects **Draft**, **Live/shared**, or **This browser**.

### 5.2 Scene list and layers

Selecting a scene cues its saved draft in Preview; selection and double-click never publish it. Scene operations include create, rename, duplicate, reorder, and delete. Duplicate copies layout/static content but reuses input tracks; it does not open another camera or create duplicate audio routes.

The Layers panel shows placements in front-to-back visual order. Add-existing-source and create-new-source are separate actions. A track may appear multiple times in one scene with independent transforms. Layer selection is distinct from global source selection.

Support rename, visibility, lock, reorder, duplicate, and removal of placements. Removing or hiding a placement does not disable its input or stop its recorder. Lock prevents accidental transform/reorder/deletion; visibility remains an explicitly labeled action. Hidden/locked items remain selectable through the Layers list.

Reject source deletion while referenced by drafts, applied Program snapshots, or active outputs; show those dependencies. Do not silently cascade-delete them. Scene deletion must not erase the currently applied snapshot or another client's unsaved work. Block deletion of the on-air scene initially.

### 5.3 Canvas and inspector

Minimum editing: drag, resize, aspect-ratio lock, crop, opacity, ordering, visibility, numeric values, fit/contain/cover, center/reset, keyboard nudging, and undo/redo. Arbitrary rotation may follow after backend validation; expose supported capabilities accurately.

Use one canonical transform model instead of independently writable scale and size values. Recommended model: canvas-space rectangle, normalized source crop, fit mode, opacity, and optional rotation about the rectangle center. Crop first, fit and clip to the destination rectangle, then apply rotation/translation and alpha. Keep canvas pixels independent of CSS pixels and account for letterboxing when hit-testing. Display pixel crop values if useful, but convert to the canonical representation consistently.

Provide numeric inputs and buttons for actions that can be performed by dragging. This also follows W3C's guidance to provide non-dragging alternatives. [R13]

One completed pointer gesture creates one undo entry. Escape cancels an unfinished gesture. Undo/redo affects draft edits only: it never reverses a Take, starts an output, changes live audio, or deletes a recording. After another client edits the same revision, reject stale undo rather than overwriting newer work.

Do not intercept shortcuts while typing, composing text with an IME, or using a modal dialog. Browser-local shortcuts are sufficient. Take is an explicit command, not a side effect of selecting a row or pressing Space during editing. Streaming start/stop has no default global shortcut. Use native controls or tested accessible components. [R17]

### 5.4 Scene save, preparation, and Take

Auto-save completed edits to the server with revision checking. Show **Saving**, **Saved draft**, **Preview preparing**, and **Program applied** separately. Pointer movement may update a transient server Preview, but Program remains pinned.

Take must use the exact committed revision identified by the UI, not “whatever is latest when the server handles the request.” Flush pending edits before preparing that revision. While a gesture or save is pending, Take cannot publish an older version silently.

The engine prepares the new layout and applies it at a defined video-frame boundary without restarting encoders or changing output format. It reports the first applied Program timestamp/revision. Validate the expected Program revision and engine-run identity as well as the target scene revision. Several property writes must not leak partially applied geometry into intermediate Program frames. On failure, keep the previous Program intact. A repeated command must not execute twice.

After success, Preview stays on the selected draft. When that draft equals Program, show **Matches Program**; further editing creates a new draft. Store the previous applied visual snapshot and offer **Cue previous Program**, which changes Preview only. Restoring it still requires Take. Cueing an older snapshot must not overwrite a newer saved draft of that Scene; editing the older version requires an explicit new draft or copy. A/B swap can be added later as a preference, not an additional first-release state machine.

Scene Take does not change microphone mute, audio routing, source timing, or an upstream player's position. Warn when taking a visually different standby/break scene will leave the current audio mix running.

### 5.5 Scope and persistence of shared controls

| Control | Application behavior |
|---|---|
| Layout, crop, visibility, static text/image content | Draft; explicit Take to Program. |
| Live gain/mute | Immediate server command, clearly labeled live. |
| Audio route creation/removal | Explicit Apply; shared live mix. |
| Source connection/format/processing | Explicit Apply, show affected users and possible interruption. |
| Source timing/common holdback | Explicit validated Apply; shared live timing. |
| Monitor mute/volume/Listen | This browser only. |
| Recording/transmission | Explicit per-output start/stop or an explicitly configured automatic policy. |

Settings reload must never implicitly start public output. Project switching while outputs are active is blocked initially. Changing the active canvas size, frame rate, or encoder format requires stopping affected outputs; do not offer an unimplemented seamless reconfiguration.

## 6. Preview and browser monitoring

Render authoritative Preview and Program on the server. The browser supplies selection handles, guides, and immediate interaction outlines; it is not a second production renderer. Temporary outlines may lead the received picture and must be labeled as pending rather than represented as confirmed pixels.

Use WebRTC for the initial low-latency preview path. GStreamer's `webrtcsink` provides multi-consumer streaming and signaling integration, but deployment still requires authenticated signaling, tested ICE connectivity, and a bounded consumer budget. [R9] Working HTTPS control does not by itself prove that the media connection works.

Preview is lower priority than production and recording. Closing the last viewer may release preview-only encoding. Opening another browser must not create unbounded encoders or change Program. Show media-disconnected/stale separately from control-disconnected; a frozen image is not a healthy live preview.

Keep server-applied revision separate from the revision currently visible in the browser. Correlate preview frames with a stream epoch and media timestamp/revision where supported. Browser frame callbacks can expose RTP timestamps, but availability and presentation timing are not universal guarantees. [R11] If reliable correlation is unavailable, report server-rendered state and **display confirmation unavailable**; do not fake it. Warn about a stale preview before Take without claiming client display time is the production timestamp.

Browser audio starts muted and requires an explicit user action. Handle playback rejection rather than treating it as a server fault. [R12] Offer **Listen: Program** or **Listen: selected source** through one private monitoring path. Source Listen uses the selected track after source processing/timing but before mix fader/mute, labeled accordingly, and never changes the live mix. Local volume and monitor mute cannot alter recordings or other browsers.

Do not diagnose source synchronization by comparing two independent browser players with different buffers. Use a combined server-rendered calibration view or a short local recording.

## 7. Input and audio workflows

Provision a named Source and its authenticated publishing endpoint, then associate incoming tracks with stable track IDs. Reconnection reuses the Source. Unknown publishers are rejected or quarantined, not automatically trusted or added to Program. A second publisher cannot silently replace an active source.

List connection state, available tracks, negotiated format, last media activity, and dependency counts. Show disconnected sources in scenes rather than deleting their placements. Resolution/timebase changes create a new continuity generation and update diagnostics; they do not silently reinterpret an old file's timestamps.

For every audio route provide gain in dB, mute, routing state, input/post-fader metering, and clipping indication. Start with one stereo master mix at 48 kHz and explicit mono/stereo conversion. GStreamer mixing requires a common sample rate, so normalize inputs rather than relying on accidental negotiation. [R6]

Do not infer audio routes from visual placements. A microphone used in several scenes enters the mix once. New audio tracks are not broadcast until explicitly routed. Scene changes do not enable or disable routes. Audio-follow-video and scene audio presets are later features with explicit semantics.

Smooth ordinary gain changes. Monitor peak/clipping and document any limiter's processing latency; do not silently normalize source levels. Prevent a failed audio input from indefinitely blocking the master clock. Silence fill, resampling/drift correction, and discontinuities must be observable.

## 8. Source synchronization

### 8.1 Required meanings

Expose a per-track **sync correction** in milliseconds. Positive values mean later playback; negative values mean earlier relative playback only when available history and common holdback make it feasible. A source-level linked control moves its video/audio corrections together, while per-track trims remain available.

Keep these separate:

| Value | Meaning |
|---|---|
| Transport buffering | Adapter buffering/retransmission/jitter configuration. |
| Sync correction | Operator adjustment relative to the normalized input timeline. |
| Program holdback | Common intentional delay allowing inputs to arrive before presentation. |
| Browser/platform delay | Downstream delivery delay; not corrected by changing capture timing. |

Do not label packet age as end-to-end capture latency. Unknown latency is unknown. Unrelated streams with independent PTS origins require explicit mapping and manual calibration; matching numeric PTS does not prove simultaneous capture. GStreamer distinguishes running time and stream timestamps; use its model rather than inventing another timing layer blindly. [R4, R7]

### 8.2 Timing invariant

One useful backend-independent model is:

```text
n_i(u) = normalized engine time for source-track timestamp u
B      = common Program holdback
c_i    = signed manual correction for track i

scheduled Program time = n_i(u) + B + c_i
```

At Program time `t`, select source material near `n_i(u) = t - B - c_i`. This defines behavior, not a prescription to apply an offset to multiple GStreamer elements. Avoid counting pipeline latency twice.

A sample must be available and retained before its scheduled use. Validate all required tracks and buffer limits. A negative correction is not permission to demand future frames. Reject infeasible requests with the additional holdback or reduced correction needed. Do not silently clamp or enlarge global latency.

Keep common holdback stable during an active run by default. An operator may explicitly change it after validation and warning. Late/missing optional inputs use their configured fallback instead of continuously increasing everyone else's delay.

### 8.3 Controls and application

The Sync panel provides video/audio corrections, linked adjustment, numeric input, millisecond/frame step buttons, reset, pending/applied values, available adjustment range, buffer occupancy, and underrun/late-frame status. Frame steps use the selected rational Program rate; display the resulting milliseconds.

Offset changes are one validated transaction. The engine reports actual application time and any discontinuity. Increasing delay may need repeated/held video or silence while filling; decreasing delay may skip material. Bound these operations and avoid indefinite stalls or uncontrolled pitch shifts. Never promise glitch-free arbitrary time jumps.

Buffer in compressed form where practical, but retain decoder dependencies, audio continuity, and honest memory limits. A timestamp property by itself is not a complete delay implementation. Measure the chosen backend path.

Clean-source recording normally taps before Program timing correction. Program recording contains the correction. Preserve the original-media-to-engine mapping and applied correction changes so the editor plugin can recover which source interval was used.

For calibration, use a visible flash/clap plus an audio transient, inspect one combined result, adjust, and verify a recording. Test both fixed offsets and long-run drift; an offset cannot fix rate drift permanently.

## 9. Outputs, recording, and recovery

Expose separate rows and controls for each recorder and network destination. Display requested versus observed state and last successful media activity. Distinguish **Sending media**, **Platform broadcast live**, and **Platform status unknown**.

A generic RTMP/RTMPS destination may go public as soon as media arrives. Therefore the UI uses **Start sending**, with a warning that it may start a public broadcast. A platform plugin may offer separate preparation and Go Live only after checking platform capability and configuration. YouTube's auto-start/auto-stop settings illustrate why this cannot be assumed. [R10]

Reconnect an interrupted output while its current desired state remains running. A manual stop cancels retries. After server restart, public outputs remain stopped unless an explicit restart policy was previously configured; restoring configuration alone is insufficient authorization. Automatic source recording on connection is a supported opt-in policy, not a general-purpose automation engine.

Record selected original/clean tracks and Program independently. Prefer remuxing compatible encoded inputs over re-encoding. A received mixed audio stream is not separate stems; advertise only the separations actually available. One recorded media file may contain multiple tracks if the catalog maps them correctly.

Use bounded, independently finalized recording segments. Validate the actual container/keyframe behavior; `splitmuxsink` splits on video keyframes and independent fragments require suitable GOPs. [R8] A closed file is not the same as a checksum-verified or decode-validated file. Preserve timestamps, track identity, tap point, format, gaps, and validation status.

Stop recording by draining/finalizing its branch. Partial files and failed finalization remain visible. Never silently delete old recordings to recover disk space. Display capacity and write failures, stop the affected recorder cleanly when necessary, and leave unrelated outputs running.

Each consumer has an explicit bounded-buffer and overflow policy. Preview may discard old raw frames; a stalled publisher must reconnect/recover at decodable boundaries; a recorder must report failure rather than indefinitely backpressuring the graph. GStreamer queues block by default when full, so adding a queue is not proof of failure isolation. [R18] Validate stall and detach behavior with fault injection.

Reference-count inputs/decoders/encoders by active consumers. Hiding a scene item, closing a browser, stopping one recorder, and disabling a plugin release only the resources they own.

## 10. Plugin contracts

Provide small typed registration points for source/output factories, commands, settings, status subscriptions, namespaced storage, and optional UI panels. Effects are added when an actual effect requires them; do not build a universal graph/plugin language.

Media extensions use the backend's native execution path. General plugins never receive raw frames through a JSON event bus and never run network/DB work in a rendering callback. GStreamer explicitly warns about blocking and state manipulation in streaming-thread callbacks. [R19]

Application plugins use the same validated commands as the UI. They cannot mutate Scene or Program objects behind the coordinator. They own their external-event schemas, retention, and platform-specific states.

Persist unknown plugin configuration when a plugin is absent. Disable unavailable features visibly rather than dropping configuration. Unloading a plugin must not terminate a user-owned recorder. Initially all code is trusted and registered at build/startup time; error containment is not a security sandbox.

Capture management is separate from receiving a stream. Platform account integration is separate from generic RTMPS output. Chat/domain data is separate from production state. If an external event causes a visual alert, the visible production effect may be observed without copying the complete external event history into the core or editor contract.

## 11. Observation and editor-session extension

The core exposes stable IDs, an engine-run ID, exact or explicitly bounded application timestamps, input continuity generations, media mappings, and recording results. Obtain a consistent state snapshot plus event cursor so changes between snapshot and subscription are not lost silently.

Use ordered per-engine events and explicit overflow/gap signals. The core need not permanently archive every event. A recording plugin subscribes before its capture interval, maintains its own journal/snapshots, and marks gaps. Runtime performance meters are best-effort and are not mixed into the reliable production-history stream.

The editor plugin coordinates existing recording outputs and stores applied scene snapshots, mixer/timing changes, markers, and media metadata. Package generation can happen entirely afterward, but information that cannot be inferred later must be observed while production runs. When export is enabled, enforce or explicitly report the absence of a Program reference.

Its minimal output contains: media inventory; clean tracks and Program reference; file-to-session time mappings; layout/source-use intervals; audio routing/automation; applied delay changes; availability/quality gaps; and editorial markers. Export reconstruction capability as editable, recorded-layer, approximate, or Program-only.

Do not require chat, donations, viewer counts, platform events, or game/application events. Other plugins may attach separate datasets by explicit integration. Do not require transcripts, live analysis, or immediate edits.

Later copied media can replace a track representation without changing logical track identity. Validate its time mapping; create a new package revision rather than silently rewriting an existing package. Deterministic replay of browsers/shaders is not a requirement.

## 12. Commands, persistence, and concurrency

Use one authoritative command coordinator. Persist completed draft/configuration edits; media runtime owns actual application status. SQLite/local files are sufficient initially. No external database, distributed broker, or microservice architecture is required.

A minimal command envelope is:

```ts
interface Command {
  id: string;
  kind: string;
  targetId: string;
  expectedRevision?: string;
  expectedEngineRunId?: string;
  payload: unknown; // Validate against the command's typed schema.
}
```

Required command families: source configure/enable; scene edit/cue; prepare/take an exact revision; live audio parameters/routing; apply timing; output start/stop; and plugin commands. Endpoint naming is an implementation detail.

Use explicit set/start/stop operations rather than ambiguous toggles. Report accepted, applied, rejected, or failed status with the actual revision/time. Deduplicate commands; for uncertain outcomes, query state before retrying. Do not claim exactly-once effects across arbitrary crashes.

Initial operation is single-operator with additional browser views. Shared Preview and Program are server-owned; panel selection and monitoring are browser-local. Use optimistic revision checks and serialize conflicting commands. On conflict, show the current state and preserve the local unsaved draft for recovery; do not silently overwrite. Full collaborative editing and operator leases are not required.

On connection loss, show stale state and disable live mutations. Keep server media running. Do not queue Take, start-output, or mute commands offline for automatic replay. Reconnect with a fresh snapshot/cursor before enabling controls. Draft text may be preserved locally, but applying it requires conflict checking.

Store assets by stable ID/version. Redact secrets from logs and project export. A private management network does not replace application authentication or publishing authentication. Platform credentials remain on the server; capture producers receive only scoped ingest credentials. Browser sources, untrusted plugins, and arbitrary remote file execution are outside initial scope.

## 13. Failure behavior and operational visibility

Show source activity, rendering cadence, encode/output progress, recording writes/capacity, and control connectivity separately. A running process or connected socket is not proof that media is progressing.

Missing sources retain their placements and IDs. Choose an explicit placeholder/hide/freeze policy; do not mislabel frozen content as a current feed. A missing input cannot block all production forever. Audio gaps produce defined silence/fallback behavior and observations.

A failed Preview branch cannot stop Program. A failed destination cannot stop recording. A full recording disk cannot silently consume unlimited memory. Catch ordinary application-plugin failures, but document that a native crash may still terminate an in-process daemon. Add process isolation only when a measured requirement justifies it.

Graceful shutdown stops new commands, finalizes recording branches, closes outputs, and records incomplete work. Native pipeline manipulation must respect data flow and EOS/drain semantics; abruptly removing a muxer can corrupt its output. [R19]

## 14. Implementation sequence

| Stage | Deliverable | Exit condition |
|---|---|---|
| A: media feasibility | Synthetic and real OBS input; clean recording; two-source composition; independent audio delay; RTMPS/test receiver; browser preview. | Timing, finalization, and branch-stall gates pass. Record exact dependencies and commands. |
| B: usable production UI | Scenes/layers, canvas inspector, draft persistence, exact-revision Take, live mixer, Sync panel, separate output controls. | End-to-end operation works from a laptop without server GUI access. |
| C: recording and recovery | Independent source/Program recording, input reconnect, storage errors, project restore, browser reconnect. | Fault tests preserve unaffected operations and expose every known gap. |
| D: extension proof | Editor-session plugin plus a small external-event/plugin panel example. | Both use public registration/command/observation boundaries, not hidden core fields. |

A relay-only prototype is an intermediate test, not the completed first release. Conversely, a plugin framework without functioning scene editing, timing, recording, and real output is not a meaningful deliverable.

Supply a README with setup, one-command local development, synthetic fixtures, OBS publisher settings, authentication, storage paths, service operation, supported codec/backend capabilities, recovery limitations, and acceptance-test results. Record unavailable hardware/platform credentials as untested integrations, not as successful tests.

## 15. Acceptance matrix

| Scenario | Required result |
|---|---|
| Edit the currently on-air scene | Program geometry/static content stays pinned until Take. |
| Take while a draft save is pending | No accidental publication of an older or different revision. |
| Take accepted but renderer preparation fails | Existing Program continues; UI reports failure. |
| Duplicate a scene or place one camera twice | No duplicate capture connection or doubled audio. |
| Hide/remove a camera placement | Its independent recording continues. |
| Change shared source timing | All affected consumers are identified; actual applied timing is reported. |
| Apply a known +250 ms correction | Composite/recorded evidence shows the intended relative delay within one output video frame plus one measured audio processing block on the synthetic no-loss fixture. |
| Apply feasible and infeasible negative corrections | Feasible correction uses retained material; infeasible correction is rejected without silent global latency changes. |
| Adjust offset during operation | Bounded transition; no permanent stall, unbounded memory growth, or hidden correction. |
| Long-running independent audio/video clocks | Drift is detected and corrected or explicitly reported; no claimed universal sync guarantee. |
| Listen to or mute browser monitoring | Program and other browsers are unchanged. |
| Take a break scene | Audio remains as configured; the UI does not imply it was muted. |
| Unknown/duplicate publisher connects | No source hijack or automatic inclusion in production. |
| Source reconnects or changes timestamps | Stable logical identity, new continuity interval, explicit gaps. |
| One publisher output stalls | Other outputs and recordings continue within documented limits. |
| Disk fills or recording is stopped | Failure/finalization status is truthful; no silent deletion or indefinite blocking. |
| Close or reconnect the browser | Media continues; stale commands are not replayed. |
| Two browsers edit the same object | Stale write is rejected; accepted state is authoritative. |
| Restart the server | Public output does not resume without an explicit configured policy. |
| Editor plugin disabled | Full basic broadcasting/recording functionality remains available. |
| Export enabled during a run | Original media, Program reference, layout/audio/timing history, and gaps can be reconciled. |
| Browser preview lags | UI distinguishes server state from visible image; no false display confirmation. |

Use generated frame numbers and audiovisual transients, network delay/loss injection, forced output stalls, and process/storage failures. Document measured sync tolerances and sustained capability on the actual deployment machine. A software test profile is not proof of hardware 1080p60 or 4K capacity.

## 16. Remaining work and authority to proceed

| Open engineering item | Next action | Blocks coding? |
|---|---|---|
| Target GPU/driver/encoder and sustained format | Run Stage A on the actual Ubuntu host; keep a software test path. | No; blocks a production performance claim. |
| Sender/plugin exposes required independent audio/video | Verify the real OBS setup. Mixed input remains mixed. | No; blocks claiming specific isolated tracks. |
| Backend buffering, live timing changes, drift | Test signed correction and bounded queues using fixtures. | No; must pass before live use. |
| Preview ICE/codecs/browser metadata | Test the target laptop and remote network; use authenticated signaling and truthful fallback labels. | No; must pass before remote operation. |
| Container/segment/codec interoperability | Verify crash/stop recovery and the intended editor import. | No; must pass before relying on archives. |
| Real platform start semantics and credentials | Run a private/test broadcast; verify auto-start behavior. | No; blocks production publication testing. |

Do not request new user decisions about themes, docking systems, package schemas, marketplaces, distributed deployment, or optional advanced features before producing the working vertical slice. Preserve the defined behavior, choose reversible defaults, and report evidence when an engineering gate fails.

## 17. Research references

References support factual observations and available primitives. The project's behavioral rules are the decisions in this document, not quotations or mandatory copies of another product. Sources were consulted on 2026-10-27; no end-to-end system was executed as part of this documentation research.

- **[R1] OBS Studio Overview — scenes and Studio Mode.** `https://obsproject.com/kb/obs-studio-overview`
- **[R2] vMix — Cut.** `https://www.vmix.com/help29/Cut.html`
- **[R3] vMix — Audio Mixer and Audio settings.** `https://www.vmix.com/help29/Mixer.html` ; `https://www.vmix.com/help29/Audio.html`
- **[R4] GStreamer — Latency design.** `https://gstreamer.freedesktop.org/documentation/additional/design/latency.html`
- **[R5] GStreamer — Compositor.** `https://gstreamer.freedesktop.org/documentation/compositor/index.html`
- **[R6] GStreamer — Audio mixer.** `https://gstreamer.freedesktop.org/documentation/audiomixer/audiomixer.html`
- **[R7] GStreamer — Clocks and synchronization.** `https://gstreamer.freedesktop.org/documentation/application-development/advanced/clocks.html`
- **[R8] GStreamer — Split mux sink.** `https://gstreamer.freedesktop.org/documentation/multifile/splitmuxsink.html`
- **[R9] GStreamer — WebRTC sink.** `https://gstreamer.freedesktop.org/documentation/rswebrtc/webrtcsink.html`
- **[R10] YouTube LiveBroadcast resource — enableAutoStart/enableAutoStop.** `https://developers.google.com/youtube/data/live/reference/rest/v3/liveBroadcasts`
- **[R11] MDN — requestVideoFrameCallback metadata and limitations.** `https://developer.mozilla.org/en-US/docs/Web/API/HTMLVideoElement/requestVideoFrameCallback`
- **[R12] Chrome — Autoplay policy.** `https://developer.chrome.com/blog/autoplay/`
- **[R13] W3C WAI — Dragging movements.** `https://www.w3.org/WAI/WCAG22/Understanding/dragging-movements.html`
- **[R14] OBS — Backend design.** `https://docs.obsproject.com/backend-design`
- **[R15] GStreamer — Rust bindings.** `https://gstreamer.freedesktop.org/bindings/rust.html`
- **[R16] OBS — SRT protocol streaming guide.** `https://obsproject.com/kb/srt-protocol-streaming-guide`
- **[R17] W3C WAI — Developing a keyboard interface.** `https://www.w3.org/WAI/ARIA/apg/practices/keyboard-interface/`
- **[R18] GStreamer — Queue.** `https://gstreamer.freedesktop.org/documentation/coreelements/queue.html`
- **[R19] GStreamer — Pipeline manipulation.** `https://gstreamer.freedesktop.org/documentation/application-development/advanced/pipeline-manipulation.html`

- **[R20] Smelter — Getting started.** `https://smelter.dev/fundamentals/getting-started`
- **[R21] Smelter — HTTP routes and RTMP input.** `https://smelter.dev/http-api/routes` ; `https://smelter.dev/http-api/inputs/rtmp`
- **[R22] Sofie — About the TV automation system.** `https://sofie-automation.github.io/sofie-core/docs/about-sofie/`
