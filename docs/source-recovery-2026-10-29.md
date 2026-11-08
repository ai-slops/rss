# Source publisher recovery candidate and native handoff

## Disposition

This is the separate opt-in recovery phase based on focused native-fix checkpoint
`2a7fdf6b741057e46aae18804dc5801804da48fe`. It is an ordinary review candidate,
not protected promotion, deployment, a broadcast authorization, or native OBS
acceptance. Root owns final independent review and protected-ref promotion.

The earlier native receipt `46eca92769dbdf47d9f79dc8ab555779a5513b8f` tested
`9fa1395c136d6a62a9782f0f836d4747035e1f15`, not this recovery implementation.
Its original OBS crash, native reconnect-provenance and owner-loss results must
be rerun against the reconciled candidate; a local pass does not relabel them.

## What the operator can use

1. In **Setup → Inputs**, open **Publisher recovery** for the owned SRT source.
   Recovery defaults off. Enable and explicitly apply the policy. Maintenance,
   disable, controller status, receiver progress, retry budget, manual retry and
   controller revocation are available here. There is no remote Start button.
2. Generate the private bundle with `scripts/target/publisher-bundle.mjs`, using
   the operator password file and `--with-recovery`. The source must already
   exist. Use `--rotate-recovery` only to intentionally invalidate the old token.
   Exact arguments are in `../integrations/obs-live-production/README.md`.
3. Transfer that mode-0600 scoped bundle privately to the OBS machine. In
   **Tools → Live Production SRT Publisher**, select it, confirm the source and
   stereo mix, then explicitly Start. Configuration loading alone stays stopped.
4. An unexpected publisher/transport stall may recreate only this independent
   Publisher. Source-scoped recovery cannot change scenes, start recordings,
   invoke arbitrary commands, or touch OBS main/public/platform outputs.
5. Manual Stop fences queued actions and persists stopped intent before stopping.
   Maintenance/disable/revocation suppress future automatic actions. A stopped
   source is never turned back on by retry. A fresh OBS/plugin process starts
   stopped/unarmed, even if a prior private state file said running.

The plugin must already be alive. No separately supervised process endpoint is
enrolled, so dead OBS/plugin restart is unavailable. The UI states this boundary;
an offline controller is not reported as a successful process recovery.

## Implemented safety boundaries

- Separate operator-cookie and per-source bearer roles. Enrollment returns a
  one-time token with no-store headers; only its SHA-256 persists server-side.
  A controller cannot retrieve ingest credentials or call operator/public-output
  APIs. Redirects and response-origin substitution are rejected.
- Monotonic policy versions, process/poll/intent epochs, server-run fences,
  command IDs and private process locks. Exact duplicate polls reuse their
  persisted result without sampling health or spending retry budget; changed
  duplicates and stale generations are rejected. Cancelled cached commands
  cannot be reauthorized by disabling and re-enabling policy.
- Clone → persist → commit transitions. SQLite write failures do not activate
  policy, change enrollment, or consume an authoritative retry in memory.
- Actual decoded video frames, audio buffers, video PTS and continuity generation
  drive health. Identical pixels do not mean frozen media. Both tracks and PTS
  must advance. Started acknowledgement opens a new three-second decode grace;
  recovery requires generation advancement and two strict A/V/PTS observations.
- At most one action is in flight, with three attempts and 1/2/4-second backoff.
  Budget, command and eligibility persist. Explicit operator retry is eligible
  only for an online armed running source and has bounded persistent deduplication.
- Local Stop, lost policy/source eligibility, server-run changes and controller
  errors cancel held actions. No previously queued reply can reverse Stop.
  The normal manual publisher is not stopped merely because automatic recovery
  is disabled; persistence failure itself disarms recovery fail-closed.
- Failed bundle/recovery configuration leaves no configured/startable Publisher.
  The native test includes a late network-configuration rejection, not only an
  early nonexistent-file case. Private state directories/files and epochs are
  atomically maintained, and integer overflow fails closed.

The exact API and behavior contract is `source-recovery-contract.md`. Qt/runtime
and private finite fault controls are in
`../integrations/obs-live-production/RECOVERY.md`.

## Executed here, not native OBS claims

All results below were produced in the isolated ordinary Alpine workspace with
GStreamer 1.26.9 / FFmpeg 8.0.1. No direct verifier-session authentication, new VM,
Studio restart, existing password/state change, deployment or public output occurred.

- Full isolated `bash scripts/acceptance.sh`, started
  `2026-10-29T16:25:16Z`, exited 0. Strict Rust/workspace checks, 38 frontend
  tests, production build/audit, 182-second drift soak, lifecycle cleanup,
  150-frame SRT strict decode, output independence, recovery, real browser
  media/control and editor gates passed. Evidence: `var/acceptance/latest.txt`.
- Parent enabled all target and recovery drivers: **57 passed, zero failed,
  zero skipped**. The real backend/GStreamer recreation receipt is
  `var/recovery-http-driver.ejdNHE/recovery-http-report.json`: generation 1 → 2,
  video 23 → 36, audio 37 → 58, advancing PTS, four strict clean segments,
  H.264 1280x720, AAC stereo 48 kHz, red motion and dominant 440 Hz. Program
  revision and output count were restored unchanged. The sender was synthetic
  GStreamer, explicitly not OBS.
- The additional fake-controller/real-media receipt
  `var/recovery-local-driver.koPHPO/recovery-local-report.json` labels its limits:
  faulted live legs use decoder counters and a separate finite leg proves full
  strict decode. It is not substituted for the real-backend recreation proof.
- Parent personally exercised the browser controls against a real isolated
  backend: `var/recovery-ui-parent.jCfLbp/report.json`, with fresh 1280/768/375
  screenshots. Enable, maintenance retry suppression, disable and revoke worked;
  policy reached version 4, no token or remote Start was rendered, no page errors
  occurred, and Program/output state stayed unchanged. The controller in that
  UI check was a source-only HTTP fixture, not OBS. Earlier independent visual
  checks used an HTTP fixture and are labelled as such.
- Six actual portable C++ test executables passed: recovery model, action handler,
  private state/lock/rotation, fault gate, fixture state, and SRT URL. Qt/libobs
  targets are registered but unrun here. A fresh nested module state directory
  and failed configuration paths are covered in native test sources.
- After final cache invalidation hardening, all nine recovery state tests and
  the dedicated HTTP integration test passed; strict workspace all-target Clippy,
  formatting and the source-size gate passed again. No failing assertion was
  removed or weakened. Final native prerequisite/script checks also pass.

The existing web chunk-size/annotation warnings and expected initial unauthenticated
401 remain visible. Relevant LSP servers are absent under the previous declined
installation; compiler, TypeScript, Clippy, tests and actual usage supply evidence.

## Same-verifier native run required

Root must relay the candidate's exact ordinary SHA to existing session
`ses_e7e47dca0ffei463kKkwhIS3az` in `live-ubuntu-validation`. Reuse the retained
guest and private evidence. Do not copy credentials or create a replacement verifier.

```sh
source scripts/ubuntu-env.sh
cargo build --locked --release --workspace
target/release/live-production-supervisor --timeout-ms 180000 --grace-ms 10000 -- \
  cargo test --locked -p live-production --lib source_recovery
target/release/live-production-supervisor --timeout-ms 180000 --grace-ms 10000 -- \
  cargo test --locked -p live-production --test http_source_recovery
npm --prefix web ci
npm --prefix web run build
cmake -S integrations/obs-live-production \
  -B integrations/obs-live-production/build -DCMAKE_BUILD_TYPE=RelWithDebInfo
cmake --build integrations/obs-live-production/build --parallel 2
ctest --test-dir integrations/obs-live-production/build --output-on-failure
cmake --install integrations/obs-live-production/build \
  --prefix "$PWD/integrations/obs-live-production/package"

scripts/target/recovery-native.sh \
  --supervisor-binary "$PWD/target/release/live-production-supervisor" \
  --server-binary "$PWD/target/release/live-production" \
  --web-dist "$PWD/web/dist" \
  --plugin-dir "$PWD/integrations/obs-live-production/package" \
  --obs-binary /usr/bin/obs --xvfb-binary /usr/bin/Xvfb \
  --evidence-dir "$PWD/var/recovery-native-new-run" --deadline-seconds 300
```

The runner invokes native Ubuntu/ABI/capability preflight. Evidence roots must be
fresh. A failed attempt records whether OBS execution actually began and never
claims OBS recovery. Here the runtime prerequisite is absent (`/usr/bin/obs`),
so no native recovery result exists for this candidate.

The native runner uses an owned ephemeral loopback UDP bridge to pause SRT DATA
while forwarding control packets and HTTP heartbeats. Explicit one-shot CONTROL
handover permits the recreated publisher's new peer/socket, rejects unknown or
old DATA, and retires the old control mapping only after decoded new-generation
progress. No payload is decoded or logged by the bridge. This is acceptance
tooling, not product network architecture.

Required real native outcomes include online media freeze/drop with recreated
publisher and strict recovered A/V, actual duplicate ID/outcome presentation,
paused dispatch with a persisted pending command cancelled by Manual Stop,
authorization failures, backoff exhaustion, source-disabled/maintenance stalls
without restart, controller/server reconnect, stale epochs, and whole-OBS offline
behavior with no process-restart claim. Require the native CTest dispatcher,
protocol, client and late-configuration regression, not only portable model tests.

Native reconnect provenance still requires the unchanged assertions and retained
session/MKVs in `../media/NATIVE_PROVENANCE_RERUN.md`. The earlier native failure
and owner-loss receipt are not relabelled by local passes. Actual native OBS
acceptance, longer target soak, laptop ICE, hardware, intended-editor import and
platform semantics remain open. Root must not promote on an HTTP 200 or on these
non-OBS local results alone.
