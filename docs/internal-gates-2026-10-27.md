# Internal recovery and synchronization acceptance

## Scope and immutable validation target

The operator's review of `3983777` correctly identified that a recording killed before stop could have raw MKV files but no catalog, and therefore disappear from startup inventory. This revision implements recovery and the remaining feasible internal gates; it does not substitute pending ledger rows for those changes.

Code checkpoint: `dea49063eb99d54f927a500d3d52216a0aec2acf`, on `work/initial-production`.

Complete acceptance started at **2026-10-27 22:48:49 UTC** in the detached checkout `var/internal-validation.TRI6kS`, using its own `target/`, `web/dist/`, state and supervised fixtures:

```sh
env -u CARGO_TARGET_DIR ./scripts/acceptance.sh
```

It exited zero. Exact command output is `var/internal-validation.TRI6kS/var/acceptance/latest.txt`. Workspace Rust tests, strict all-target Clippy, formatting, source-size limits, 33 frontend tests, Biome, TypeScript, build, npm audit, media/network/output/browser/editor gates all passed. The full gate now requires `output-crash-recovery-gate.sh`; admission, epoch, clean provenance and three-minute endurance tests are part of normal workspace tests.

The running studio was not rebuilt, restarted, reconfigured or deployed. Supervisor PID `419348` and application PID `419353`, application start ticks `36675901`, remained unchanged, with no deleted executable. After fixture cleanup, it was the only project application process. Its served bundle remained `index-BkJ4ow-m.js`; the isolated validation bundle was `index-sLiX4w22.js`. No service operation, public transmission, automatic merge or deployment was performed.

## Abrupt process death: files remain discoverable

Implementation:

- Atomically write/fsync `session.json` before recording starts, and atomically replace/fsync catalogs.
- Publish only actual `splitmuxsink-fragment-closed` notifications. Filename order is not a closure signal under async finalization.
- At startup enumerate raw fragments even when catalogs are missing, corrupt, unreadable or omit later files. Only a nonexistent root means an empty inventory; permission/I/O errors remain errors.
- Recompute checksum/decode evidence. Preserve richer new observations; prior catalog values are fallback only and cannot promote partial files to complete.
- Keep interrupted tails, zero-byte files, missing-file entries and explicit repair/interruption gaps visible. Never delete or rewrite raw MKVs during recovery.
- Report Recovery, Validation and Mapping separately. `recovered` does not mean decode-validated, and `decoded` does not invent a missing clock mapping.
- Restore every output desired/actual state as stopped. Inventory and editor export include the recovered files and interruption gaps.

Actual isolated process regression: `var/internal-validation.TRI6kS/var/output-crash-recovery.tVpiT0/`.

The fixture waited for two confirmed closed fragments plus an active tail, sent SIGSTOP to freeze writes for the before-hash snapshot, then SIGKILL without finalization. A new daemon reopened the same isolated state.

Observed result:

```json
{"closedBeforeKill":2,"exportInventory":3,"fragments":3,"hashesPreserved":true,"outputsStopped":true,"recoveries":["partial","recovered"]}
```

All three hashes matched before kill, after kill and after restart:

```text
segment-00000.mkv ac4836daca1a1717fe36078e9a1d94e64eb8a380504e8522fa57c7824ef41dd9
segment-00001.mkv ed9dc6c7b2154581e3b8a1afc2b5a8b4071da18550f8d5b9ca6d6fbcd83da8c8
segment-00002.mkv e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
```

The first two files were recovered/decoded; the interrupted zero-byte tail remained partial/unmapped. `abrupt_shutdown` and missing-fragment gaps reached export. Catalog/session repair changes metadata only. The gate removes its disposable authentication cookie jars during cleanup.

## Simultaneous publishers and bounded failed handshakes

Source admission now uses native socket-address peer identities with `Vacant`, `Pending(peer)` and `Active(peer)` leases. Another caller's rejection/removal cannot release the lease. Expiration never transfers a still-live pending handshake to another peer.

The authenticated publishing endpoint returns the source-scoped `streamId` as well as its passphrase. New provisioned senders should supply both. Wrong stream IDs are rejected before reserving a lease. Existing empty-streamid clients retain the same peer-specific, non-reassigning admission policy.

An actual wrong-passphrase/correct-streamid test found that native SRT did not emit a rejection/removal callback and otherwise left the slot pending. The media owner now closes/detaches the old listener before rebuilding it after the bounded pending deadline; it does not release a live reservation from a callback. Measured recovery was about `2.0–2.1 s`.

Real senders start from a barrier. Red and green have each won separate runs; only the winner becomes accepted or supplies media. The loser is either explicitly rejected or remains in the native transport backlog without admission or media authority. A later blue reconnect advances continuity to generation 2. Tests also cover accepted red → disconnect → failed pending authentication → owner recovery → valid blue, requiring generation 2 for both video and audio. Counter high-water, not the last queried snapshot, determines the next generation.

## Captured clean provenance, not current-state inference

Native reference metadata carries engine run, source, track, media kind and authorized continuity generation. Generation authorization is serialized by the media owner:

```text
old generation → block ingress → FlushStart/FlushStop → authorize new generation → rebase/resume
```

Rendering probes never read a newly accepted admission counter to relabel old decoder buffers. Already-tagged consumer queues retain their old generation. Failed flushes remain blocked and cause owned source recovery. PTS reset starts a fresh normalized epoch without moving engine timestamps backward.

The real queued-media test observed red/440 Hz in generation 1, then blue/880 Hz in generation 2 for video and audio. Clean recording spans from both generations, availability gaps, stable source/track IDs and engine-run identity were persisted to session/catalog metadata and exported unchanged after reload. Program recordings do not invent clean-source provenance.

Fragment clipping uses proportional interpolation between observed local/media endpoints, including drifted rates and gaps; invalid reversed ranges are rejected rather than clamped. Nanosecond values serialize as decimal strings. `9007199254740993` round-trips exactly, and earlier numeric catalog values remain readable. Missing legacy provenance remains unknown rather than being assigned the current connection.

## Sustained signed timing and drift reporting

`srt_drift_endurance` runs for at least three minutes, applies three signed correction transactions, reconnects with a different clock profile, checks a bounded 4096-sample observer window, samples RSS/queues, and finalizes independent Program recordings. It explicitly routes source audio and checks actual recorded pixels and PCM, not just a running clock or decodable black video.

Focused integrated run of the committed code tree:

| Observation | Result |
|---|---:|
| Duration | 182 s |
| Program frame growth | 5,410 |
| Decoded nonempty recording segments | 37 |
| Recorded nonblack pixels in probe | 2,801 |
| Recorded PCM RMS | 2,791.09 |
| Positive encoded timestamp injection | +1,500 ppm |
| Receiver video/audio rate estimate | +1,499 / +1,500 ppm |
| Reconnect sender wall-pacing target | -12,000 ppm |
| Receiver reconnect video/audio estimate | -9,073 / -7,842 ppm |
| Reconnect video/audio arrival skew | -88,787,531 / -70,015,719 ns |
| Reconnect video/audio observation window | 8,803,582,478 / 8,831,909,920 ns |
| Video/audio regression standard error | 95 / 65 ppm |
| Video/audio timestamp residual RMS | 3,916,892 / 3,328,046 ns |
| Maximum video/audio queue occupancy | 767,825,700 / 940,081,506 ns |
| Warmup-relative RSS growth | 84,396 KiB |

All queues remained below their four-second bounds and RSS growth below the fixture's 512 MiB ceiling. The detached full acceptance independently passed the same endurance regression in 182.76 s.

Timing changes, with 500 ms common holdback:

1. Video -250 ms, audio +125 ms.
2. Video +250 ms, audio -100 ms.
3. Video -100 ms, audio +200 ms.

These estimates compare original decoded ingress PTS against monotonic receiver arrival, before manual correction. Regression standard error and residual RMS are fit statistics, **not** conservative clock bounds or original capture latency. MPEG-TS PCR policy affects the relationship between injected pacing and receiver observation. Reporting is implemented and visible in the API/Sync panel; universal drift correction is not claimed. This measured three-minute software profile is not a multi-day soak or hardware throughput guarantee.

## Lifecycle and browser checks

A native test process initially segfaulted after successful assertions because process exit raced an unjoined media owner. All engines now retain and join their owner threads, detach sources and set the core graph to NULL; recorder owners separately drain their muxers. Core EOS waiting is not used to stall shutdown. Native tests and full process acceptance exited cleanly after this fix.

The full browser gate at `var/internal-validation.TRI6kS/var/browser-acceptance.gnivw3/` passed actual routing/unrouting, selection-only clicks, sequential numeric editing, immediate Escape, two-view stale-undo protection, explicit live timing, exact Take, private selected-source audio, both decoded monitors and page-close continuity. Initial unauthenticated 401 is expected; no autoplay/ICE policy bypass was enabled.

Separate real recovery/drift/provenance UI proof and 375/768/1280 captures are under `var/internal-ui-20261027-1930/recovery-evidence/`. Recovered, partial, mapped/unmapped, measured drift and Unknown states are distinct, and private recording paths are not rendered.

The retained original engineering reviewer approved the corrected internal gate implementation. Original owner sessions/models were reused; no replacement team or worker was created. Rust LSP remains unavailable by the existing install decision; strict compiler/Clippy checks passed.

## Remaining limits

External hardware/Ubuntu sustained capacity, real OBS/plugin track separation, actual platform account semantics/private broadcast, target-laptop remote ICE, and intended editor import remain unverified where those systems were unavailable. No public broadcast or deployment was performed. Native in-process plugin crashes remain possible; the tested recovery path preserves their recording evidence rather than claiming crash isolation.
