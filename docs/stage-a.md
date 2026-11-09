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
