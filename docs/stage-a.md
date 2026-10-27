# Stage A media feasibility

## Scope and boundary

The media implementation is the standalone Rust package in `media/`. It uses the official
`gstreamer` Rust crate and keeps every GStreamer object on a dedicated media-owner thread.
The coordinator calls `MediaEngine::spawn(EngineConfig)` and sends typed `Command` values
through `MediaEngine::execute`. No GStreamer type crosses that boundary.

Commands carry a stable command ID and optional expected engine-run ID. Events carry the
command ID, engine-run ID, ordered decimal-string sequence, and decimal-string nanosecond
application time. Duplicate command IDs return the previous event without repeating the
effect. Output snapshots separate desired-running state from observed state. Signed timing is
validated before application: an infeasible negative correction reports the additional
holdback required, and a correction exceeding the five-second fixture buffer is rejected.

The current owner controls named GStreamer `valve` elements independently, maps stable track
IDs to source timestamp controls, and applies validated route/gain/mute values to mapped
`audiomixer` pads. `PreparePreview` applies an exact scene revision to the
Preview compositor at a blocked output-frame boundary. `TakePrepared` rejects stale or
unprepared revisions, then applies the same placement set to Program at its next blocked frame
boundary and reports the resulting Program revision. The public Rust types are in
`media/src/protocol.rs`, the handle is in `media/src/engine.rs`, and backend ownership is split
between `engine_owner.rs`, `engine_commands.rs`, and `engine_layout.rs`.

## Reproduction

Project scripts default to two compilation jobs.

```sh
./scripts/bootstrap.sh
. ./scripts/env.sh
CARGO_BUILD_JOBS=2 cargo test -p live-production-media
./scripts/media-stage-a.sh
./scripts/media-network-gates.sh
```

The scripts write only under `var/`. `scripts/media-stage-a.sh` executes the Rust binary,
decodes every finalized file with FFmpeg, and measures the two-track timing fixture.
`scripts/media-network-gates.sh` runs a loopback SRT/MPEG-TS sender/receiver and a controlled
loopback RTMP/FLV receiver.

## Measured environment

Measured on 2026-10-27 in the project-local Alpine sysroot:

- Rust and Cargo `1.91.1`.
- GStreamer `1.26.9`; Rust crate resolved to `gstreamer 0.24.5`.
- FFmpeg `8.0.1`.
- Available elements: `compositor`, `audiomixer`, `splitmuxsink`, `x264enc`, `voaacenc`,
  `matroskamux`, `srtsrc`, `srtsink`, `rtmpsink`, `webrtcbin`, and `webrtcsink`.
- `webrtcsink` is from `gst-plugin-webrtc 0.14.3-RELEASE`.

## Results

The exact Stage A command was:

```sh
. scripts/env.sh && CARGO_BUILD_JOBS=3 cargo run -p live-production-media -- \
  stage-a --output-dir var/media-stage-a-final --duration-seconds 6 \
  --holdback-ms 500 --video-correction-ms=-250 --audio-correction-ms 125
```

Observed results:

- Program composition finalized three approximately two-second Matroska segments. Every
  segment decoded without FFmpeg errors and contained H.264 `1280x720` video plus stereo AAC
  at `48 kHz`.
- Preview and Program are separate server compositor elements with independent geometry. The
  Stage A Preview sink is deliberately slowed and discardable; it does not feed Program.
- The independent clean recorder finalized three approximately two-second Matroska segments.
  Every segment decoded without FFmpeg errors and contained H.264 `640x360` video plus stereo
  AAC at `48 kHz`.
- `splitmuxsink` segment durations were `2.005333`, `2.005333`, and `2.033333` seconds for
  both recorders. Final segment size was nonzero and all six files passed full decode.
- The Program pipeline includes a preview consumer sleeping `500 ms` per buffer behind a
  one-buffer downstream-leaky queue. Program and clean recordings still advanced and
  finalized, demonstrating bounded isolation for this controlled stalled-consumer fixture.
- A two-track AAC/Matroska timing fixture requested `+250 ms`. First packet PTS values were
  `0.000 s` and `0.250 s`, a measured delta of exactly `250.000 ms`.
- The timing validator accepted `B=500 ms, video=-250 ms, audio=+125 ms`; it rejected
  `B=100 ms, correction=-250 ms` with `150 ms` additional holdback required and rejected a
  scheduled delay `100 ms` beyond its configured buffer.
- The loopback SRT gate received a `4.876178 s`, `808,964` byte MPEG-TS file containing H.264
  `640x360` video and AAC audio. FFmpeg decoded it without errors.
- The controlled loopback RTMP gate received a `5.002 s`, `744,483` byte FLV file containing
  H.264 `640x360` video and AAC audio. FFmpeg decoded it without errors.
- The WebRTC gate ran for three seconds, registered the `webrtcsink` producer with its embedded
  signaling server, and finalized cleanly with signaling port `8443` and web port `8080`.
- The Rust test suite passed 13 tests, including live GStreamer owner-thread scenarios that
  apply per-track timing and mixer changes, open an output valve, prepare Preview, Take the
  exact prepared revision at a frame boundary, and read authoritative state.

## Implemented media paths

- Separate synthetic two-video Preview and Program compositions with explicit geometry.
- Two independently generated audio inputs normalized to a stereo `48 kHz` mix.
- Signed per-track correction plus explicit common holdback on the measured synthetic path.
- Independently finalized clean and Program recorders with bounded two-second segments.
- SRT/MPEG-TS H.264/AAC input description and loopback transport/decode gate.
- Independently supervised H.264/AAC FLV publishers fed by native Program taps; controlled local
  RTMPS and two-destination gates are recorded in `docs/output-evidence.md`.
- Browser-preview description through `webrtcsink`, with embedded signaling and web servers.
- Typed, deduplicated owner commands with engine-run checking and ordered event metadata.
- Exact-revision Preview preparation and frame-boundary Program Take with stale-revision
  rejection.
- Live audio route, gain, and mute updates addressed by stable track ID.

## Latest integrated validation

The full acceptance run started on 2026-10-27 at 14:34:21 UTC against code checkpoint `ef2e70d` passed all workspace tests, all-target strict Clippy, browser media, output and extension gates. Its fresh Stage A artifacts are `var/media-stage-a.7r3dHH/`; calibration again measured exactly `250.000 ms`. The result details and remaining partial gates are tracked in `docs/acceptance.md`; historical measurements above are not target-machine throughput claims.

Dynamic compositors now keep an engine-owned hidden live input so adding the first scene layer does not reset an otherwise padless compositor's output timeline. Native video taps preserve engine-run-scoped reference timestamps. A decoded multi-fragment recording test reconciles actual visual transitions with authoritative Program Take PTS. Timing transactions also have a paused/resume regression proving failed commands cannot apply later.

## Explicit gaps

- Preview/Program layout, source add/remove/reconnect, per-track timing, live mixer values, and
  named output valves are dynamically controlled through run-scoped native taps. Browser and
  recording media remain isolated native GStreamer consumer branches.
- No external OBS instance was supplied. The SRT fixture proves the expected encoded network
  profile and decode path, not OBS-specific publishing controls or independent OBS tracks.
  Controlled wrong-secret rejection and stable-ID reconnect are tested separately.
- Controlled local RTMPS receivers now validate TLS transport, reconnect/manual-stop behavior,
  and two-destination isolation. A real platform account, its auto-start semantics, and a private
  platform broadcast remain untested.
- Project-local Chromium now decodes both server-rendered monitors and selected-source audio
  through authenticated same-origin signaling with normal media policies. Remote-laptop/shared-
  ingress ICE remains unverified, and reliable browser frame/revision correlation is not claimed.
- Signed timing now uses timestamp normalization, bounded decoded history and scheduling probes
  for the controlled synthetic/SRT profile. The authenticated SRT test observes retained decoded
  video/audio and rejected infeasible timing. Sustained independent-clock drift, arbitrary sender
  profiles and long-running transition/memory stress remain unverified.
- Recorder, preview, and publisher isolation are measured. Publishers have independent retry and
  manual-stop lifecycles; storage failure and graceful `SIGTERM` recording recovery are covered in
  `docs/output-evidence.md`.
- Hardware encoding, sustained 1080p60/4K capacity, long-run clock drift, abrupt `SIGKILL` recovery,
  and intended editor import compatibility were not tested.
