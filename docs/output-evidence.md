# Recording and network output evidence

## Implemented ownership

Each configured output has a dedicated owner thread, GStreamer consumer pipeline, bounded queues,
desired/observed state, media-progress timestamp, and independent retry/finalization lifecycle.
Program recording consumes the typed, engine-run-namespaced
`live-<engine-run>-program-{video,audio}-v1` taps. Clean recording consumes the paired
pre-timing/pre-Program `live-<engine-run>-clean-{video,audio}-v1-<track-id>` taps. A restarted
engine therefore cannot cross-connect an old output consumer to a new graph.

Recording owners encode H.264/AAC into bounded Matroska segments. Stop drains EOS, waits for
finalization, computes SHA-256, fully decodes every closed segment with GStreamer, writes a
`catalog.json`, and exposes stable catalog IDs plus capture bounds through the coordinator
snapshot. An empty recorder stopped before its first encoded buffer stops without inventing a
recording.

Network owners encode the same native Program taps to FLV and pipe them to an independently
supervised FFmpeg RTMPS publisher. Publisher progress is parsed from FFmpeg's machine-readable
`total_size` progress stream, so `lastMediaAtNs` reflects the network publisher rather than merely
the upstream encoder. A failed publisher is detached and retried without stopping recording or
another destination. Manual stop cancels retries. Destination URLs are stored in the
server-side SQLite `output_destinations` table through the authenticated
`POST /api/v1/output-destinations` endpoint; project configuration and snapshots contain only the
destination ID. FFmpeg runs with quiet logs at the secret-bearing boundary and errors exposed to
clients are sanitized.

Duplicate starts are idempotent and keep one owner/session. On graceful daemon shutdown, the
coordinator sends an explicit shutdown request, recorders drain/finalize, publisher children stop,
and every owner thread is joined. On restart, completed catalogs are loaded from disk, while every
restored output has desired and actual state `stopped`; no public publisher is resumed merely
because its configuration exists.

## Exact verification commands

```sh
. scripts/env.sh
CARGO_BUILD_JOBS=3 cargo test -p live-production-media --test output_contract
CARGO_BUILD_JOBS=3 cargo test -p live-production --test http_outputs
CARGO_BUILD_JOBS=3 cargo test -p live-production --test http_recovery
CARGO_BUILD_JOBS=3 cargo test --workspace
CARGO_BUILD_JOBS=3 cargo clippy --workspace --lib --bins -- -D warnings
sh scripts/output-acceptance.sh var/output-acceptance-final3
```

The latest full `./scripts/acceptance.sh` run, started on 2026-10-27 at 14:34:21 UTC against code checkpoint `ef2e70d`, passed all
workspace tests and strict all-target Clippy. Its output artifacts are under
`var/output-acceptance/`; it finalized and decoded eight recording fragments, passed two controlled
RTMPS receivers and stall isolation, restored three finalized segments after graceful shutdown,
and drove the rendered output controls. The `var/output-acceptance-final3/` paths below retain
the earlier detailed measurement and should not be confused with this latest run.

New catalogs also carry bounded per-file engine-media mappings when native producer reference
timestamps are available. They do not reinterpret the older broad session wall bounds as file
PTS. The real cross-fragment transition proof and legacy/unmatched-run behavior are described in
`docs/extensions.md`.

## Recording gate

The authenticated HTTP gate created one synthetic source, one Program recorder, and one clean
source recorder through the same command API used by the UI. It started both outputs together,
ran for three seconds, stopped each independently, fetched the final snapshot, recomputed every
catalog checksum, and decoded every file.

Observed under `var/output-acceptance-final3/recording/`:

- Seven finalized Matroska segments: three Program and four clean-source segments.
- Program files: H.264 `960x540` plus stereo AAC `48 kHz`.
- Clean-source files: H.264 `1280x720` plus stereo AAC `48 kHz`.
- Segment durations were bounded near one second; the final clean drain fragment was `0.166666 s`.
- Every catalog entry had nonzero bytes, a matching 64-character SHA-256, `validation=decoded`,
  a nonzero capture interval, and the correct `program` or `clean_source` tap point.
- FFmpeg completed decode for every segment. Its null muxer emitted one non-monotonic DTS warning
  on the short final drain fragment; GStreamer full-decode validation succeeded and the gate exited
  zero. The warning is retained here rather than hidden.

Primary artifacts:

- `var/output-acceptance-final3/recording/final-snapshot.json`
- `var/output-acceptance-final3/recording/state/recordings/*/*/catalog.json`
- `var/output-acceptance-final3/recording/state/recordings/*/*/segment-*.mkv`
- `var/output-acceptance-final3/recording/server.log`

## Controlled RTMPS and independence gate

The network gate generated a localhost certificate, placed the local CA only in the publisher
process environment, and ran two independent TLS-fronted RTMP receivers. The production output
manager published the Program tap to both RTMPS destination IDs. The stall phase terminated the
primary TLS receiver while leaving the secondary receiver untouched.

Observed under `var/output-acceptance-final3/network/`:

- Healthy phase: both destination snapshots were `running` with observed media activity.
- Stall phase: primary snapshot was `retrying`; secondary remained `running` with media activity.
- Healthy primary: H.264 `640x360`, stereo AAC `48 kHz`, `6.904 s`, `236889` bytes.
- Healthy secondary: H.264 `640x360`, stereo AAC `48 kHz`, `6.912 s`, `237248` bytes.
- Stalled primary retained a decodable `1.904 s`, `64709` byte partial FLV.
- Unaffected secondary continued to a decodable `16.904 s`, `576229` byte FLV.
- All four FLV files passed full FFmpeg decode and have checksums in `sha256.txt`.

Primary artifacts:

- `var/output-acceptance-final3/network/healthy-report.json`
- `var/output-acceptance-final3/network/stall-report.json`
- `var/output-acceptance-final3/network/healthy-primary.flv`
- `var/output-acceptance-final3/network/healthy-secondary.flv`
- `var/output-acceptance-final3/network/stall-primary.flv`
- `var/output-acceptance-final3/network/stall-secondary.flv`
- `var/output-acceptance-final3/network/sha256.txt`

## Fault and restart gates

`media/tests/output_contract.rs` proves:

- bounded recording segments finalize, checksum, and decode;
- an unavailable publisher retries while an independent recorder remains running;
- manual network stop cancels retry without stopping the recorder;
- an unwritable recording root fails only that recorder while another recorder continues;
- a mid-session recorder failure retains closed/partial catalog entries in memory;
- duplicate starts keep one owner and one recording session;
- manager drop drains active recorders and joins their threads.

`server/tests/http_recovery.rs` proves command deduplication and that restored output configuration
returns stopped after an engine/coordinator restart. `server/tests/http_outputs.rs` proves
authenticated secret provisioning, secret-free snapshots, Program recording, and native clean
source recording through coordinator commands.

`scripts/output-shutdown-gate.sh` sent `SIGTERM` while a Program recorder was active. The daemon
exited zero after finalizing three decoded segments. A fresh daemon using the same state directory
restored all three catalog entries and reported the output as desired/actual `stopped`. Evidence is
under `var/output-acceptance-final3/shutdown/`.

## Browser command gate

`scripts/output-browser-gate.sh` drove a real project-local Chromium against the authenticated
production server. Through the rendered Outputs panel it created and ran/stopped a Program
recorder, created a clean-source recorder from the source-ID selector, and created an RTMPS output
using only the provisioned destination ID. The RTMPS URL was provisioned separately through the
authorized server endpoint and never appeared in the DOM. Fresh `1280x900` and `375x812`
screenshots show the controls without overlap or horizontal overflow.

Artifacts:

- `var/output-acceptance-final3/browser/outputs-1280.png`
- `var/output-acceptance-final3/browser/outputs-375.png`
- `var/output-acceptance-final3/browser/server.log`
