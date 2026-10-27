# Initial production execution

Baseline: the complete `implementation-handoff.md`, read on 2026-10-27.
Standalone project: Live Production. Work branch: `work/initial-production`.

## Execution and ownership

1. Stage A: provision project-local development dependencies; execute actual synthetic encoding/decoding, composition, signed timing, independent finalization and stalled-consumer tests. Establish a Rust/GStreamer media-owner boundary. Real OBS and external platform gates require equipment/credentials not supplied here.
2. Stage B: one authenticated daemon, SQLite configuration, serialized typed commands and revision checks; React operator workspace with separate Draft/Live/Browser scope and authoritative server media.
3. Stage C: independent clean/Program recorders and network publishers, source continuity, bounded consumer queues, restart safety, catalog validation and browser recovery.
4. Stage D: trusted typed registrations, reliable observation cursor/gaps, editor-session journal/export and one external-event example through public commands.
5. Exercise complete workflows and controlled faults, obtain independent engineering and browser reviews, address actionable findings, publish recovery checkpoints and expose the authenticated development interface via shared ingress.

## Constraints

- No host/control-plane/router reconfiguration; dependencies and checkpoints stay in this workspace.
- No credentials in source, logs, browser URLs or evidence. Ingest credentials are scoped; platform credentials remain server-side.
- No public media destinations or claims of real hardware/OBS/platform validation without observations.
- Media graph execution is separate from coordinator/database work. Preview failure, browser closure and one failed output must not govern production lifetime.
- No implicit public-output start on config load or restart. No offline replay of live commands.
- Short ADRs record measured deviations, rather than hiding unsupported behavior.

## Evidence

Exact tool versions, commands, observations and limitations will be recorded alongside each engineering gate. A passing software fixture establishes only its measured software profile, not hardware throughput or remote-network ICE connectivity.
