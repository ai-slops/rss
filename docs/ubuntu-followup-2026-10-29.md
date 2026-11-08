# Ubuntu findings: primary follow-up and verifier handoff

## Exact scope and ownership

This ordinary branch, `work/ubuntu-followup-20261029`, starts from primary
checkpoint `22e7951757ddf3b0d85ed6d23363a2c2f33cee4c`. That clean checkout remains
unchanged. The running studio checkout, state, executable, served bundle, and
process generation are unchanged. There is no merge, deployment, or public output.

The published verifier receipt was read from exact
`a4026e083548ee058a3a1dd2d0015528393198ea`, checked out separately for inspection.
Its real Ubuntu 24.04.5 / GStreamer 1.24.2 / FFmpeg 6.1.1 / OBS 30.0.2.1 / Qt 6.4.2
results apply to old `77389b4` plus its verification tooling, not the new OBS
plugin or rehearsal harness. Its failures remain failures until rerun.

Operator-authenticated coordination owns delivery to the existing
`live-ubuntu-validation` session `ses_e7e47dca0ffei463kKkwhIS3az`. No further
cross-workspace authentication attempts, copied Basic credentials, replacement
verifiers, VM provisioning, or native environment installation were performed.
The retained runtime, outputs, UI, and independent reviewer sessions were reused.

## Concrete changes

- Adopted verifier commit `3d2d031ad061bd347a2e7a7fae6cdfbdf098e07d` as
  `4531b8379571dd2475367696cd3e9239128db0a0`: explicit
  `LIVE_PRODUCTION_NATIVE=1` activation for the existing runners, rejecting an
  inherited sysroot. Default Alpine behavior is unchanged. VM tooling is not
  copied into the product branch.
- Corrected the installed OBS frontend CMake target to `OBS::obs-frontend-api`.
  Upstream OBS 30.0.2 uses `OBS::frontend-api` only as its build-tree alias;
  its installed export is the real target with the `OBS::` namespace.
- Require 48 kHz stereo in both the native Tools control and publisher core.
  Tools cleanup runs on OBS EXIT, callback removal precedes unload cleanup, and
  QAction destruction clears the controller's pointer. Start/Stop remain explicit;
  a restart never starts publishing automatically.
- Added `scripts/target/publisher-bundle.mjs`: existing SRT input only, observed
  source/track IDs, scoped publishing endpoint, private descriptor-validated
  operator-password file, and exclusive mode-0600 bundle creation. Remote ingest
  host must be explicit; it is not guessed from the HTTPS proxy. Operator
  credentials never enter the OBS bundle. API redirects are rejected so a 307/308
  cannot forward login credentials to another origin.
- The SRT network fixture now requests I420, Annex-B/AU H.264, encoder-owned
  repeated headers, and parser reinsertion. Receiver validation requires
  H.264 640x360 plus stereo 48 kHz AAC and still performs full strict decode.
- Added a real intervideo memory-identity diagnostic and opt-in preservation of
  finalized `session.json` plus every MKV before provenance-dependent assertions.
  No product provenance-restoration code is added and no assertion is weakened.

## Native findings and next owners

| Finding | Current status | Owner and next action |
|---|---|---|
| Missing `webrtcsink` | Native dependency failure remains open | Existing Ubuntu verifier supplies/reports native rswebrtc, then records `gst-inspect-1.0 webrtcsink` and its actual version |
| Daemon exits before readiness | Missing required preview backend is a plausible explanation, not confirmed from the missing private log | Verifier retains the actual daemon log and reruns native lifecycle after dependency correction; primary has not weakened readiness |
| Clean recording lacks video provenance | Not fixed or relabelled passed | Primary runtime owner awaits the raw artifacts and identity diagnostic below; verifier runs the unchanged assertions and returns evidence |
| SRT H.264 0x0 / missing PPS / decode 69 | Shared fixture corrected; native result pending | Verifier reruns the actual network gate, retaining strict decode and NAL evidence |
| New OBS plugin | Concrete installed-target, lifecycle, stereo, and setup fixes prepared; native build/load still pending | Verifier builds this branch and exercises automated fixture plus native Tools controls |

### Why the provenance workaround was withheld

The first candidate restored metadata by raw-frame SHA-256 matching and short
history. Independent review rejected it: identical camera pixels across reconnects
can be assigned the wrong generation, and a GAP flag does not distinguish a
repeated source frame from generated timeout-black. All of that candidate's
runtime code was removed before publication.

The replacement investigation does **not** restore metadata. Its local real
intervideo run, with producer render completion and matching caps, reported
different producer/consumer sysmem objects (`same_layout=false`). That is local
GStreamer 1.26.9 evidence, not a finding about the target's 1.24.2 path. The exact
native loss point is still unproven. Pixels, arrival order, GAP flags, and the
current connection must not be used to manufacture source-generation intervals.

Run the two bounded commands and retain all artifacts specified in
`../media/NATIVE_PROVENANCE_RERUN.md`. Missing metadata stays unavailable until
an exact buffer identity or a metadata-preserving transport is demonstrated.

## Commands for the existing native verifier

Fetch this ordinary branch through the normal repository channel and record its
actual HEAD. Do not overwrite the original evidence or boot a second VM.

```sh
LIVE_PRODUCTION_NATIVE=1 CARGO_BUILD_JOBS=2 cargo build --locked --workspace

cmake -S integrations/obs-live-production \
  -B integrations/obs-live-production/build -DCMAKE_BUILD_TYPE=RelWithDebInfo
cmake --build integrations/obs-live-production/build --parallel 2
ctest --test-dir integrations/obs-live-production/build --output-on-failure
cmake --install integrations/obs-live-production/build \
  --prefix "$PWD/integrations/obs-live-production/package"

LIVE_PRODUCTION_NATIVE=1 bash scripts/media-network-gates.sh var/native-network-new-run
LIVE_PRODUCTION_NATIVE=1 bash scripts/lifecycle-acceptance.sh

scripts/target/rehearsal.sh \
  --plugin-dir "$PWD/integrations/obs-live-production/package" \
  --evidence-dir "$PWD/var/native-obs-new-run" \
  --server-binary "$PWD/target/debug/live-production" \
  --supervisor-binary "$PWD/target/debug/live-production-supervisor" \
  --obs-binary /usr/bin/obs
```

The rehearsal evidence root must not already exist. Its preflight must fail if
native capabilities are absent; bypassing preflight is not acceptance. The native
Tools/bundle operator procedure is in
`../integrations/obs-live-production/README.md`. Also test non-stereo rejection,
Stop during Starting, close/reopen, active OBS exit, and restart staying stopped.
Keep all OBS logs and publisher bundles private.

For the SRT file, return `ffprobe` dimensions/profile, full `ffmpeg -xerror`
decode status, and extracted H.264 SPS/PPS/IDR NAL inventory. This distinguishes
startup loss from missing later header reinsertion. Transport bytes alone are
not usable-video proof.

## Executed local evidence

These are Alpine/musl, GStreamer 1.26.9 / FFmpeg 8.0.1 results, not Ubuntu passes.

- Full isolated `bash scripts/acceptance.sh`, started
  `2026-10-29T05:44:59Z`, exited 0: strict Clippy/formatting, Rust suite including
  182-second drift soak and the new diagnostic, 33 UI tests, UI build/audit,
  lifecycle, Stage A calibration at 250.000 ms, SRT/RTMP full decode, output
  independence, crash recovery, browser media/control, and editor gates.
  Evidence: `var/acceptance/latest.txt`.
- Actual corrected SRT receive in that run: 4.885333 seconds, 777944 bytes,
  H.264 640x360, AAC stereo 48 kHz, full decode passed, at
  `var/media-network.92hidd`. Existing RTMP listener shutdown prints an I/O-error
  message; its captured media subsequently passes strict decoding.
- After final credential/artifact hardening, all **35** target/network checks
  passed with both native drivers and export decoding enabled. These include
  cross-origin redirect refusal, symlink/oversize password rejection, concurrent
  bundle-path preservation, and actual HTTP/synthetic-SRT workflow. Script-text
  header checks are configuration regressions only, not native compatibility.
- Strict workspace Clippy and formatting passed again after those final edits.
  The unchanged reconnect/provenance assertions passed with artifacts retained
  at `var/provenance-parent-qa`: session JSON and both MKVs. The first MKV is
  H.264 320x240 / AAC stereo 48 kHz and full decode exited 0.
- The parent personally used the new bundle CLI against an isolated real daemon:
  `var/bundle-parent-qa.KiHKDn/report.json` records observed IDs, mode 0600,
  credential-free stdout, operator-password absence from the bundle, existing
  output preservation, and help behavior. The first manual attempt correctly
  encountered the existing 500-ms login cooldown after fixture setup login;
  the successful run respected that precondition rather than weakening auth.
- Native Qt/libobs compilation and rendered OBS controls could not be exercised
  here. Compiler-independent upstream/export checks are not a native pass.
  LSP servers remain unavailable under the previously declined installation.

The existing greater-than-500-kB web chunk warning and dependency annotation
warnings remain. No thresholds or assertions were relaxed. Target laptop ICE,
TURN/host UDP mappings, intended-editor import, longer native soak, physical
capture, hardware encoding, and platform broadcasts remain unverified.
