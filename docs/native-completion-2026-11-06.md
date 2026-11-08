# Executed native Live verification, 2026-11-05/06

## Disposition

**Native acceptance is still failed.** This receipt records executed software
checks, corrected verifier defects, and reproducible remaining failures. Neither
published source, native compilation, HTTP 200, sender declarations, nor passing
control-only probes promote the failed media gates.

The machine-readable companion is
`native-completion-manifest-2026-11-06.json`. It is a fixed-field sanitized
manifest, not a copy of private enrollment/authentication or process state.
Root owns final integration/publication. This work remains a **local ordinary
branch and bundle**; no main merge, push, force push, deployment, or broadcast.

## Exact source and retained environment

Fetched normally and verified both authorized checkpoints:

- `work/live-native-fixes-20261029`:
  `2a7fdf6b741057e46aae18804dc5801804da48fe`.
- `work/source-recovery-20261029`:
  `9a07ef4beda93505a4c0d50485bc3c5f9df5df62`.

Git proved the native-fix checkpoint is an ancestor of the recovery checkpoint.
The new local `work/ubuntu-native-completion-20261105` worktree therefore
fast-forwarded to the latter. No assumption about main/config alignment was used.
All original branches, indexes and evidence remain preserved. No `AGENTS.md`
was present in the checked project/hierarchy or tracked feature tree; the README
and requested native/provenance/recovery handoffs were read.

Native execution checkpoints:

| Run | Exact code | Purpose |
|---|---|---|
| Initial | `e81489d583ce6347a4d85fd9e55c51f2845a3115` | Fresh isolated build, CTest, original failures and full recovery entrypoint |
| Corrected Rust/browser | `014599ed87dba9fd12dc3d6ab7064938b5cf7f64` | Packetized remaining test senders, full Rust suite, native browser |
| Media retention/RSS | `16aca0ec6f9501038be75843f272867d26a1d55e` | Preserve failing recordings before assertions/decoding |
| Final canonical native run | `1228a8d4a612e2b959eff64ff8b3722f84233b08` | Original production/plugin blobs, corrected validation, live safety profiles |
| Final naming-only cleanup | `2e3e76c98ad662f60b632e71f41d40b402f117e6` | Unit-verified receiver-freeze terminology; no production/runtime change |

The final `integrations/`, `server/src/` and `media/src/` trees were verified
byte-identical to the supplied recovery checkpoint. Failed private fixture clock
and activation experiments were reverted. They remain in local history/raw
evidence as negative experiments, **not proposed production fixes**.

The same retained VM/overlay/backing/key/pinned SSH identity was resumed serially
for bounded runs, never replaced or run concurrently with another VM. KVM/q35,
host CPU, 4 vCPU/8192 MiB; workspace budget remained 4 CPU/12 GiB/8192 PIDs.
No host directories, Docker sockets, operator credentials or capture devices
were mounted. Only loopback SSH forwarding was used; all fixtures ran inside
the guest. No existing Studio, recordings, account, or other workspace changed.

Actual environment: Ubuntu **24.04.5**, kernel **6.8.0-147-generic**,
Rust/Cargo **1.91.1**, Node **24.18.1**, npm **11.16.0**, GStreamer **1.24.2**,
native rswebrtc **0.14.3-RELEASE**, FFmpeg **6.1.1-3ubuntu5**,
OBS **30.0.2.1-3build1**, Qt **6.4.2**. Playwright **1.56.1** installed its
Chromium **141.0.7390.37**, build **1194**, with native Ubuntu dependencies.
No DIM APT mirror is selected; public dependency downloads/APT were guest-only.

The managed launcher failed before QEMU on workspace boot-ID access
(`ENOTCONN`). No host mount/service workaround was attempted. Its owned
registration was removed, and the reviewed foreground VM runner supplied the
deadline and exact-child cleanup instead. This was not missing KVM access.

## Results actually executed

| Native software gate | Result | Private evidence |
|---|---|---|
| Locked debug/release build; native Qt/libobs link/stage | Pass | `var/completion/results.tsv` |
| CTest, including partial-init state and Qt dispatcher teardown | **10/10 pass** | `var/completion-08/ctest.log` |
| Owner-loss, exact PID/start generation, reaping before strict ESRCH | **8 tests pass**, 11 native outcomes + 7 process-tree cases | `var/completion/lifecycle.log` |
| SRT network profile and complete receiver file | **150/150 video frames**, unchanged strict decode passes | `var/completion/media-network.log` |
| Native source epoch, SRT admission and signed timing | Pass after sender packetization correction, assertions unchanged | `var/completion-04/rust-workspace.log` |
| Actual Chromium/local webrtcsink | Pass: two 960x540 videos, three connected peers | `var/browser-completion-01/media-stats.json` |
| Target/configuration/drivers | **63 pass, 0 fail, 0 skipped** | `var/completion-07/target-drivers.log` |
| Frontend | **38 tests pass**, production TypeScript/Vite build passes | `var/completion/web-tests.log`, `web-build.log` |
| Final format/strict all-target Clippy | Pass | `var/completion-08/results.tsv` |
| Genuine OBS expected audio content | **Fail, exit 1** | `var/completion-08/obs-rehearsal.log` |
| Genuine OBS full recovered-media workflow | **Fail, exit 1** | `var/completion-08/recovery-full.log` |
| Reconnect clean provenance | **Fail, exit 101** | `var/completion-04/rust-workspace.log` |
| Bounded Program recording | **Fail, exit 101** | `var/completion-05/program-recording.log` |
| 180-second drift/RSS recording | **Fail, exit 101** | `var/completion-05/drift-rss.log` |

The native browser run used Playwright inside the VM, not an external browser
substitute. It decoded Preview/Program, received four packets on the separate
private audio peer, exercised actual controls, kept shared routes unchanged
under private Listen, and preserved Program after page closure. The expected
initial unauthenticated 401 is retained. No autoplay or ICE-policy bypass was
enabled. This proves local software behavior, **not target-laptop/external ICE**.

The 63 passing target tests include real native GStreamer and live backend HTTP
drivers, decoded recording content and portable package integrity/decoder checks.
Their synthetic/fake-controller labels remain intact. They do not substitute
for genuine OBS audio or actual editor application import.

## Genuine OBS: fixed crash, failed signal

The original null-Publisher failure is no longer reproduced: native state and
dispatcher tests pass, the real module loads, both primary SRT senders remain
`sending`, and actual negative output signals produce `badAuth:rejected` and
`duplicateSender:rejected`. Initialization failure remains a different outcome.
These are real OBS process/plugin and authenticated input observations.

But full receiver/recording validation still rejects the expected audio:

```text
Program RMS:        0.000005815293113410987
Recovered clean:   0.0000036283144492204986
Fixture floor:     0.001 (-60 dBFS RMS)
```

The fixture generates 440/880 Hz tones entirely in software; no physical audio
device is required, so absent hardware cannot waive this failure. Video reaches
the decoder/recorders and Program layout checks, but AAC content is numerical
silence rather than the declared tones. The publisher/fixture/integration path
needs its existing owner's correction; a definitive internal cause is not
claimed. Source generation must not be inferred from pixels to hide this.

The old relative-only spectral check sometimes labelled numerical AAC noise a
dominant tone. Those older OBS `initialStrictMedia:pass` rows are **superseded**,
not acceptance evidence. The corrected harness requires overall RMS and an
absolute normalized amplitude for **every** expected tone, retains the original
relative/noise tests, and rejects loud 440 Hz masking absent 880 Hz. The quiet
false-positive regression failed before the correction and now passes; real
GStreamer positive controls also pass under the stronger checks.

Private fixture-only clock and active-reference experiments did not restore
audio. Mixed-domain, coherent absolute-clock, and activation trial artifacts
are retained; final plugin code is restored to the authorized checkpoint.
Do not cherry-pick those experimental commits as fixes.

## Actual native recovery safety probes, not full media acceptance

The coupled workflow was not allowed to skip failed media and call itself a
pass. Separate explicit diagnostic profiles exercised later independent cases
with the real OBS module and backend. They always report **`status:partial`,
`obsClaim:false`, exit 1**, and preserve `initialStrictMedia:failed`.

Latest canonical reports:

- `var/recovery-completion-21/recovery-report.json`: missing/wrong/wrong-source
  authorization 401; queued manual Stop cancels a nonempty held command without
  execution; stopped remote retries fail 409; actual OBS SIGKILL becomes offline;
  a fresh normal OBS PID does not resume backend media; stale process/poll/intent
  requests fail 409; changed duplicates fail while exact duplicates return the
  same response; fresh process registration cannot arm from a running payload;
  controller rotation rejects the old token with 401.
- `var/recovery-completion-22/recovery-report.json`: three actual native restart
  attempts, exhaustion, 1/2/4-second eligibility, duplicate concurrent manual
  retry resetting budget once, one held action and Stop cancelling it. Backoff
  checks now include lower bounds with a declared 500 ms sampling tolerance,
  plus the upper bound; near-zero backoff is rejected by regression.
- `var/recovery-completion-23/recovery-report.json`: controller reconnect,
  offline/online lease expiry, maintenance suppressing actions during real
  stalled receiver data, source-disable suppression with maintenance **off**,
  and server restart preserving budget/fencing action execution while under
  maintenance. Program revision/output counts remain unchanged.

The recreated-generation protocol requests above are real HTTP probes to the
running native backend, not claims that the replacement normal OBS process
loaded/enrolled a new controller. `freshObsNoBackendMediaResume` intentionally
does **not** claim that its Tools UI was inspected as stopped. Whole OBS process
restart capability is unavailable/not enrolled; killing OBS is not reported as
successful automatic process recovery.

### Freeze boundary

The primary's private `freezePublisher` operation returns
`externalProcessRequired` without actually freezing a publisher. Pausing SRT DATA
in the owned bridge is a real receiver data freeze, but uses the **same bridge
mechanism** as the drop case. It is not an independent physical/publisher fault.

The verifier names that case `receiverDataFreeze`, rejects the old ambiguous
omission name, and records the common mechanism. A future otherwise-successful
full report remains partial with
`requiredUnavailable:["publisherTransportFreeze"]` until a genuine supported
hook/target is supplied. Whole-process SIGSTOP/offline behavior is a separate
boundary. No unsupported freeze is silently waived.

## Remaining native recording failures

1. **Clean reconnect provenance:** the unchanged test observes active generations
   `{1}` instead of `{1,2}` at `server/tests/editor_clean_provenance.rs:100`.
   Full workspace execution also retains the native SIGSEGV during unwind. The
   session/MKVs are preserved before provenance-dependent assertions. The exact
   identity diagnostic still reports `same_layout=false`; its metadata stripping
   is fault injection, not evidence permitting a relay/workaround.
2. **Program recording:** `runtime_recording_contract.rs` requires at least two
   bounded segments but obtains one 6870-byte recovered/unmapped fragment with
   `recording_failed`. Its complete bytes/session are retained. Earlier shorthand
   “Program Take pixels” was imprecise: the failing assertion is segment count.
3. **Drift/RSS recording:** the 180-second run advances 5409 Program frames and
   records observed RSS growth **36028 KiB**, max video queue **801203363 ns** and
   audio queue **961460101 ns**. Nevertheless, recording fails early and its
   second fragment is empty. The full soak remains failed; later drift/content
   assertions are not claimed passed merely because these observations are
   bounded. The retained session is `state:failed` with only about 0.54 seconds
   between its wall start/end, not a successful 180-second recording.

Raw preserved media under `var/completion-05/media/`:

| File | SHA-256 | Strict decode |
|---|---|---:|
| Program `segment-00000.mkv` | `1f3f17f2bc5a8a66212f018371800e19ba633abcec35c8d66d1f9d48510e087a` | 0 |
| Drift `segment-00000.mkv` | `3a84a80043b7e476fb305ab916c621e27ce8c28bc104f1b3486748264cd2b108` | 0 |
| Drift empty `segment-00001.mkv` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | 183 |

Full FFprobe JSON, decoder stderr/statuses and session metadata are present.
The empty file's FFprobe exits 1 and reports EBML header failure. No tail was
trimmed, interval invented, failure deleted or assertion weakened.

## Focused verifier corrections and replay

- Completed `alignment=7` on the four remaining Rust test senders. Before that,
  several contracts received audio but zero video; unchanged epoch, admission
  and timing assertions now pass.
- Added finite SRT drain to local recovery calibration and strengthened its
  frame count to exactly 150. Its original strict corruption failure is retained.
- Added native Chromium selection without Alpine library/font overrides;
  default Alpine behavior is unchanged.
- Preserved raw failed media/RSS before assertions/decoder failure, with
  environment opt-in and enforced `0700` directories / `0600` files.
- Added private timeout progress and independent partial probes. Every gate
  keeps its original full acceptance conditions.
- Corrected completion wrappers to propagate failed gates and manual decodes.
  Historical rounds 2–5 initially returned zero after recording failed TSV rows;
  those shell zeros are **not** treated as passes. Final wrappers exit 1 and
  capture evidence before the VM cleanup path.
- Retained reviewer verified the five prior false-acceptance/safety findings
  were corrected. Its final naming issue was fixed and unit-tested. This is
  scoped verifier review, not production approval; root's review remains required.

Relevant replay entrypoints are `scripts/ubuntu-completion-guest.sh` and the
numbered `ubuntu-completion-round*.sh` guest runners. Each result TSV includes
command label, exit code and UTC start/end. Host runners transfer clean Git
bundles, collect private evidence even on failure, and execute inside
`timeout ... scripts/ubuntu-vm.sh resume ...`. Fresh private evidence roots are
required; never overwrite this run's roots to manufacture a replay.

Minimal failed gate commands, inside the retained guest after native setup:

```sh
source scripts/ubuntu-env.sh
scripts/target/rehearsal.sh --plugin-dir "$PWD/var/obs-plugin-stage" \
  --evidence-dir "$PWD/var/NEW-obs-run" \
  --server-binary "$PWD/target/release/live-production" \
  --supervisor-binary "$PWD/target/release/live-production-supervisor" \
  --obs-binary /usr/bin/obs --deadline-seconds 120

target/debug/live-production-supervisor --timeout-ms 180000 --grace-ms 10000 -- \
  cargo test --locked -p live-production --test editor_clean_provenance -- --nocapture
target/debug/live-production-supervisor --timeout-ms 180000 --grace-ms 10000 -- \
  cargo test --locked -p live-production-media --test runtime_recording_contract -- --nocapture
target/debug/live-production-supervisor --timeout-ms 300000 --grace-ms 10000 -- \
  cargo test --locked -p live-production-media --test srt_drift_endurance -- --nocapture
```

Set `LIVE_PRODUCTION_PROVENANCE_EVIDENCE_DIR` and
`LIVE_PRODUCTION_MEDIA_EVIDENCE_DIR` to fresh owner-only paths when replaying
to preserve files before TempDir cleanup. Original assertions stay enabled.

## Private evidence, runtime and next authority

All raw artifacts remain private/ignored in `var/` and in the stopped guest's
`/home/validator/completion/var/`. Archives include credentials, databases and
raw media: **never publish them**. Only this sanitized receipt/manifest and
reviewable tooling enter the local Git bundle.

Final private archive:
`/workspace/main/var/ubuntu-validation/native-completion-evidence-08.tar.gz`,
SHA-256 `7567a369008851f14913c47fa6cb59ac4220b3d5cb9be80e36eeca0505b8abec`.
Media-retention archive `native-completion-evidence-05.tar.gz`, SHA-256
`f92f36318d507dbc52d5e8f93d17474bdb49a2d4c61e28c31c387dc0f56afe89`.
All earlier failed/counterfactual runs remain retained as separate archives.

The final exact QEMU PID **18229** stopped at **2026-11-06T00:39:24Z**; it is
absent, the owned lock is available, workspace service list is empty, and no
automatic boot/retry is scheduled. The retained overlay and SSH identity remain.
The final runner exit **1** accurately represents failed/partial gates. Backing
image SHA-256 remains
`6a81c37564db9b1ee84e141922625e1d7c5b389b99bb3c572e0243607d5bb4d2`.

Root should relay the exact signal, provenance and recorder failures to the
existing primary owner for new ordinary correction checkpoints. No new VM,
operator credentials or public platform start is needed for those software
corrections. The verifier did not simultaneously change production code.

Separately unavailable human/physical acceptance still needs a specified actual
laptop/OS/browser and network/approved UDP or TURN path, real capture devices and
hardware/driver profile, an intended editor/version/application for import, and
later explicit provider/private-broadcast authorization plus a test target.
Software Xvfb, local ICE and portable decode are not substitutes. No public or
provider broadcast was started, and none is authorized by this receipt.

LSP cannot inspect this separate worktree outside the original request cwd;
no clean LSP result is invented. Native compiler, strict Clippy, formatting,
unit regressions and actual process/browser/media use provide the recorded
evidence. No model/provider/auth configuration was rewritten.
