# Native follow-up diagnostic receipt, 2026-10-29

## Failures first

**This follow-up is not native acceptance.** The unchanged provenance test still
fails because its persisted mixed fragment has no video interval. Both plain
lifecycle runs still fail the owner-loss ESRCH assertion. The primary's updated
SRT gate still receives undecodable H.264. Genuine OBS acceptance remains
blocked pending the implementation owner's separate null-Publisher fix.

No provenance heuristic, source restart implementation, assertion relaxation,
product rewrite, protected merge, deployment or public broadcast was performed.

## Exact sources and integration boundary

- Fetched and verified ordinary primary branch `work/ubuntu-followup-20261029`
  at **`1c8d8adffb8d1c48f8b7c2cc39e38e7009f1d7a0`** through the existing scoped
  origin. Read its follow-up handoff and `media/NATIVE_PROVENANCE_RERUN.md` first.
- New isolated worktree: `/workspace/ubuntu-followup-validation`, branch
  `work/ubuntu-followup-validation-20261029`.
- Initial build and unchanged diagnostic/test runs:
  **`9cef59477f387753951be44c8ad1087ba268dd86`**.
- Read-only lifecycle observation, primary SRT and bundle/security tests:
  **`345c726ae86ebd22b68f9ec98a8134bca9979414`**.
- Final evidence-tool/source-attestation checkpoint:
  **`b18f3945c1cda4202be84f2839718149f8a0178a`**. Only receipt documentation
  follows this checkpoint.
- Product, OBS plugin, provenance/identity tests, lifecycle checks, native env
  entrypoint and SRT network script were verified byte-identical to the exact
  primary checkpoint. The only preflight addition is the previously reviewed
  `timeoverlay` capability requirement.
- Reused only non-overlapping Ubuntu VM/dependency/environment tooling from the
  previous verifier. The primary already adopted the native-env and installed
  CMake export fixes, so those were not duplicated.
- Original checkpoints/worktree indexes remain preserved:
  `work/ubuntu-validation` at `a4026e083548ee058a3a1dd2d0015528393198ea` and
  `work/ubuntu-obs-integration-20261028` at
  `46eca92769dbdf47d9f79dc8ab555779a5513b8f`.

### Shared SRT fix overlap

This run deliberately did **not** mix the previous verifier's `alignment=7`,
finite-source and drain changes into the primary's new I420/Annex-B/header
profile. It tests the latter as published. The old verifier's passing
150-frame/full-decode receipt remains preserved on `46eca92`; the primary-only
profile's native failure below is the evidence for root to relay when the
implementation owner chooses the canonical combined fix. No competing SRT
product implementation or silent conflict resolution was introduced here.

## Same retained Ubuntu VM

The existing private overlay, immutable backing and pinned SSH identity were
resumed, not replaced. Budget stayed at KVM/q35/host CPU, 4 vCPU / 8192 MiB,
userspace NAT, loopback-only SSH `127.0.0.1:22428`. No host directory, Docker
socket, operator credential, model setting or other workspace was accessed.

Actual versions: Ubuntu **24.04.5**, kernel **6.8.0-142-generic**,
Rust/Cargo **1.91.1**, Node **24.18.1**, npm **11.16.0**, GStreamer **1.24.2**,
FFmpeg **6.1.1-3ubuntu5**, OBS **30.0.2.1-3build1**, Qt **6.4.2**,
native rswebrtc **0.14.3-RELEASE**. Existing verified native dependency artifacts
were copied within the guest into the new checkout; previous binaries/evidence
were not overwritten. No APT mirror is selected; no packages or host settings
were changed in this run.

Guest boot ID: `6ff05907-f045-4660-9370-57b345850106`.

## Commands and results

From the guest checkout, after sourcing `scripts/ubuntu-env.sh`, all native
execution used the repository supervision path and bounded deadlines:

```sh
cargo build --locked --workspace
cmake -S integrations/obs-live-production \
  -B integrations/obs-live-production/build -DCMAKE_BUILD_TYPE=RelWithDebInfo
cmake --build integrations/obs-live-production/build --parallel 2
ctest --test-dir integrations/obs-live-production/build --output-on-failure
cmake --install integrations/obs-live-production/build \
  --prefix "$PWD/integrations/obs-live-production/package"

LIVE_PRODUCTION_NATIVE=1 CARGO_BUILD_JOBS=2 \
target/debug/live-production-supervisor --timeout-ms 180000 --grace-ms 10000 -- \
  cargo test --locked -p live-production-media \
  --test intervideo_provenance_identity -- --nocapture

LIVE_PRODUCTION_NATIVE=1 CARGO_BUILD_JOBS=2 \
LIVE_PRODUCTION_PROVENANCE_EVIDENCE_DIR="$PWD/var/followup/clean-provenance" \
GST_DEBUG='referencetimestampmeta:7,intervideosrc:7' \
target/debug/live-production-supervisor --timeout-ms 180000 --grace-ms 10000 -- \
  cargo test --locked -p live-production --test editor_clean_provenance \
  clean_recording_reconnect_provenance_survives_catalog_reload -- --nocapture

LIVE_PRODUCTION_NATIVE=1 CARGO_BUILD_JOBS=2 \
LIVE_PRODUCTION_LIFECYCLE_EXPECT_ZERO=1 bash scripts/lifecycle-acceptance.sh

node scripts/observe-native-lifecycle.mjs "$PWD/var/followup/lifecycle-observed"
LIVE_PRODUCTION_NATIVE=1 bash scripts/media-network-gates.sh \
  "$PWD/var/followup/network-primary"
```

| Check | Actual result |
|---|---|
| Locked native workspace build | Exit 0 |
| OBS native compile/stage and CTest | Pass, CTest 1/1; not a load/streaming claim |
| Format and strict all-target Clippy | Exit 0 |
| Exact intervideo identity diagnostic | Exit 0, `same_layout=false` |
| Unchanged clean provenance test | Exit 101, missing video interval at support `mod.rs:49` |
| Plain native lifecycle, twice | Exit 1 both times, missing ESRCH at `native-lifecycle.test.mjs:41` |
| Read-only observed lifecycle | Exit 0, 11 outcomes restored zero baseline; instrumentation-sensitive, not a fix |
| Primary SRT network script | Exit 1 at its received-video profile check |
| Separate SRT strict decoder command | Exit 69 |
| Primary bundle/security/network contract tests | 8 passed, zero skipped; configuration/security coverage, not native media acceptance |
| Private bundle CLI `--help` | Exit 0 |
| NAL evidence-tool regressions | 3 passed on host and guest |

The lifecycle assertions and all primary test bodies were unchanged. A passing
observed run does not override either failed plain run.

## Provenance evidence and the measured loss boundary

The complete test output includes the `persisted clean fragment` JSON. The
opt-in preservation hook ran after recorder stop and before provenance-dependent
assertions, retaining `session.json` and **both** MKVs.

The complete session has exactly two provenance intervals: **audio generation 1
and audio generation 2**, plus one reconnect gap. The printed first fragment
also has audio-only intervals. The unchanged video-track assertion fails. There
was no unwind SIGSEGV/SIGABRT in this particular run; earlier failures remain
preserved and are not treated as repaired.

| File | Bytes | Duration | Actual streams | Strict decode |
|---|---:|---:|---|---:|
| `segment-00000.mkv` | 56915 | 3.008000 s | H.264 320x240, AAC stereo 48 kHz | 0 |
| `segment-00001.mkv` | 39924 | 2.133333 s | H.264 320x240, AAC stereo 48 kHz | 0 |

Decoded signal statistics cover **90 + 64 = 154 video frames**. Every frame has
Y minimum, maximum and average **16**. The simultaneous GStreamer debug log
repeatedly reports:

```text
gst_inter_video_src_create:<recording_video_input> Creating black frame
```

Thus this run does not demonstrate valid captured moving-ball video whose
metadata alone vanished: the recording consumer is generating black fallback.
The confirmed boundary is **audio provenance is present, while the recording
video consumer supplies fallback rather than observed captured video**. Exactly
where the actual input video stops arriving upstream is not yet proved by these
probes. The separate SRT failure below is compatible with an earlier delivery/
decode problem, but that cross-fixture causal link is not claimed as established.
Do not manufacture video generations for these black frames.

### Exact memory diagnostic

Ubuntu 1.24.2 reports `same_layout=false`:

- producer memory `0x7c81f401bd00`;
- transported memory `0x7c81f81d29b0`;
- both `GstAllocatorSysmem`, `parent=None`, size 384, maxsize 391, alignment 7,
  offset 0, flags 0.

The diagnostic's rendered-white assertion passed. Its metadata removal is
**intentional fault injection**, not observation of real production metadata
loss at that pad. Different ordered memory objects with no parent relationship
do not justify an exact-memory-identity relay. Pixel/arrival/GAP/current-connection
heuristics remain withheld, as the primary handoff requires.

## Native SRT: received NAL evidence

The primary-only updated script still produces H.264 **0x0**, unknown profile,
alongside AAC-LC stereo 48 kHz. `ffprobe` exits 0 while reporting those unusable
video dimensions; strict `ffmpeg -xerror` exits **69** with missing-PPS errors.
Attempted FFmpeg Annex-B file extraction exits **234** and creates an empty
output. That failed attempt is retained, not counted as a valid inventory.

To inspect the demuxed received bytes without requiring a decoder or valid
extradata:

```sh
ffprobe -v error -select_streams v:0 -show_packets -show_data \
  -show_entries packet=pts_time,dts_time,flags,data -of json \
  var/followup/network-primary/srt-received.ts \
  > var/followup/analysis/network-srt.packets.json
node scripts/inspect-h264-nals.mjs \
  var/followup/analysis/network-srt.packets.json \
  > var/followup/analysis/network-srt.nals.json
```

The structural inventory covers **133 demuxed video packets / 553381 payload
bytes**:

| NAL type | Count |
|---|---:|
| SPS (7) | 0 |
| PPS (8) | 0 |
| IDR (5) | 0 |
| Non-IDR slice (1) | 532 |
| Access-unit delimiter (9) | 133 |

These parameter sets and IDRs are absent throughout the **demuxed payload**,
not merely its first sample. This is a structural byte inventory, not decoding,
frame-content validation, a complete TS continuity analysis, or provenance
restoration. Its regressions cover mixed 3/4-byte prefixes, split-packet start
codes, absent parameter sets, and malformed dumps.

**Root handoff:** relay this result with the old `46eca92` packetization/drain
evidence to the primary owner for one canonical SRT fixture change. Keep the new
profile/header requirements and strict checks; do not silently select a passing
branch or relax decoding.

## Lifecycle observation, without weakening the gate

The new observer launches the unchanged lifecycle script under existing
supervision and records only `/proc` PID generation, state and parent changes.
It does not intercept signals, inspect arguments/secrets, change assertions, or
kill unrelated processes.

Both ordinary runs fail the same ESRCH check. In the separate passing observed
run, the last native process, PID **9246**, start ticks **42643**, transitions:

```text
09:34:34.909Z  S     parent 9235
09:34:35.440Z  Z     parent 9235
09:34:35.476Z  gone
```

This supplies a concrete short-lived zombie/reaping transition and shows the
result is timing-sensitive under observation. It does **not** prove the PID was
a zombie at the precise assertion in either unobserved failing run. Parent/
generation/reaper ownership and the gate's observation race remain for the
primary supervisor owner to resolve. Do not declare a universal leak or a fix
from this one observed pass.

## Private artifacts actually returned

Extracted files are available in this verifier worktree at **`var/followup/`**,
not merely summarized. The same files remain inside the stopped guest at
`/home/validator/followup/var/followup/`. Contents include:

- `clean-provenance-rerun.log`, complete debug/test output and printed fragment;
- `clean-provenance/session.json` and both finalized `segment-*.mkv`;
- full per-file FFprobe JSON, strict decode logs/statuses and signal statistics;
- full identity layouts in `intervideo-identity.log`;
- both plain lifecycle logs, observed lifecycle log/state transitions, and
  lifecycle fixture server logs;
- received TS/FLV, failed extraction, FFprobe packet bytes, NAL inventory;
- exact source attestation, versions, command results and SHA-256 manifest.

These raw files stay private and ignored by Git. No private receipt archive,
media, SSH key/disk, login/ingest credential or token is committed/pushed.

Private archive:
`/workspace/main/var/ubuntu-validation/ubuntu-followup-evidence-20261029.tar.gz`

SHA-256: `d4834d80cfa0b5c67ef942b709e011c1075eb4cd9b1c1f9f04d8cb1cc042dcb3`.

Selected integrity hashes:

| Artifact | SHA-256 |
|---|---|
| `clean-provenance-rerun.log` | `0a9178f04e7164d8db29a4f3e7fec5da383c94a76bccd58a9eeebaf74d4f1f51` |
| `session.json` | `9d92266ecb9b6bcdda2c392f23d336d55ccd0a17b63c9aab87c773fc17660277` |
| `segment-00000.mkv` | `ede46b337afbc69bbdd3f0ff7810f2f55bd09c5eab9a5de910479612c2568c0d` |
| `segment-00001.mkv` | `0c3b501b8d51a5572a7b592c5b3f95b1b61c0f80a39eed1921dcfbf61362a7b7` |
| `intervideo-identity.log` | `4210a20f1fba971c264b88af23d312dfb76a9c1c5672834539d184feb1f7b0a2` |
| `network-srt.nals.json` | `9c8f79ecc0e39ffaef6471c7be4f71ed965b334c9f16c451e976bc63f36d366c` |
| `srt-received.ts` | `6471a9c59c1e593a4e1dcaeb39d393715a4c3dc7b992154f14f3c6f1e731b96d` |

## Runtime stopped and remaining owners

The same VM ran **09:27:26Z to 09:45:04Z**, QEMU PID **8241**, then stopped
through its managed ownership path. PID 8241 is absent and the
`ubuntu-verifier` service registration was removed; workspace service list is
empty. The runner's final 143 is the requested stop, not a test result. The
immutable backing SHA-256 still matches
`6a81c37564db9b1ee84e141922625e1d7c5b389b99bb3c572e0243607d5bb4d2`.
The overlay, SSH identity, old evidence and caches remain retained. No automatic
boot/retry is scheduled.

Genuine OBS null-Publisher repair and automatic remote source restart remain
**primary-owned**, awaiting later exact checkpoints. No genuine OBS acceptance
or remote-restart/drop-recovery pass is claimed here. Target laptop ICE, physical
capture/hardware encoding, intended editor choice/import and public platform
behavior remain unverified. No current Studio or public output was manipulated.

Node syntax/unit checks and native compiler/Clippy checks passed. LSP is unable
to inspect this isolated worktree outside the original request cwd; no clean
LSP result is claimed. Root's independent review and owner-coordinated integration
remain required before any merge.
