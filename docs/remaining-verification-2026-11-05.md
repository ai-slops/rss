# Remaining verification and recovery run binding

Historical receipt: subsequent native failure corrections and replay requirements
are recorded in [native production corrections](native-production-corrections-2026-11-06.md).
The failures retained below are not erased or converted into passes.

## Delivery boundary

This is a local correction on `work/remaining-verification-20261105`, based on
`9a07ef4beda93505a4c0d50485bc3c5f9df5df62`. Root owns independent review,
integration and publication. No protected merge, push, deployment or public output
is authorized by this receipt. The existing native verifier retains Ubuntu/OBS
and native harness ownership; this change does not edit that harness.

The worktree is `/workspace/main/var/remaining-verification-20261105`. Artifact
paths below are relative to it. Local verification uses Alpine/musl, Rust 1.91.1,
GStreamer 1.26.9 and FFmpeg 8.0.1, not the native Ubuntu/OBS environment.

## Corrected production defect

A pending recovery action from a previous daemon could accept a late `started`
acknowledgement after restart. Subsequent receiver generation numbers belong to
the new engine run, but could incorrectly satisfy the old action's health test
and reset the retry budget. The regression first failed with `Started` where
`Cancelled` was required.

Recovery records now persist an optional engine-run identity. Startup binds all
records before source initialization or receiver observations; direct polling
also binds before duplicate handling. A changed run cancels in-flight/started
actions and clears receiver baselines, decode grace, healthy-observation count,
controller last-seen state and cached poll responses. Controller/token identity,
process/poll/intent high-water marks, armed manual intent, retry count/backoff and
manual request history remain intact. A queued manual retry without an issued
command remains pending. Legacy records without the optional field are rebound
without trusting old health.

All rebound rows are committed in one SQLite transaction before replacing live
manager state. A failing second-row write rolls back both database and manager.
New records inherit the current binding. Public recovery response shapes and the
source-only authorization boundary are unchanged.

Coverage includes old pending acknowledgements, old duplicate rejection, already
started actions, legacy records, remaining retry budget and failed batch writes.
The retained Oracle approved the correction for local commit/bundling, not native
acceptance or integration.

## Passed software checks

| Check | Observed result | Receipt |
| --- | --- | --- |
| Full isolated acceptance | Passed, including strict Clippy, formatting, workspace tests, 182.84-second drift soak, media/network, recordings, crash recovery, browser and extensions | `var/acceptance/latest.txt` |
| Recovery Rust cases | 13 passed within the workspace suite | Same acceptance receipt |
| Web | 38 tests passed; TypeScript/build and Biome passed; audit found zero vulnerabilities | Same acceptance receipt |
| Strict process lifecycle | 8 tests passed; 11 native outcomes returned to baseline, including owner loss | `var/remaining-lifecycle-20261105/lifecycle.log` |
| Portable C++ | 6 optimized strict-warning executables passed; 4 UBSan executables passed | `var/portable-cpp-verification/{portable-cpp-summary,ubsan-summary}.log` |
| Real HTTP restart scenario | Old action cancelled, duplicate rejected, late ACK ignored, attempts preserved at 1, next action attempt 2; manual Stop cancelled; output count and Program scene unchanged | `var/recovery-run-http.imeMjo/report.json` |
| Real HTTP recovery with synthetic SRT | Passed in the extended target run, including recreated sender and actual decoded recording evidence | `var/recovery-http-driver.KNkLoO/recovery-http-report.json` |

The acceptance header reports base HEAD `9a07ef4`; it ran with the uncommitted
Rust correction now delivered on this branch. It is not a claim that the exact
unmodified base passed the new regression. No Rust source changed afterward.
Rust LSP is unavailable and installation was previously declined; compiler and
strict Clippy evidence are available, but an LSP-clean result is not claimed.
Existing non-fatal web build warnings concern Zod comment annotations and the
greater-than-500-kB JavaScript chunk. The browser receipt includes the expected
unauthenticated 401, not a credential-access workaround.

Reproduce controlled acceptance with:

```sh
source scripts/env.sh
umask 077
GIT_MASTER=1 bash scripts/acceptance.sh
```

The evidence archive also retains `var/recovery-run-qa.mjs`, the executed manual
HTTP scenario. It creates its own private state and credentials, stops and
restarts only its owned server, and makes no OBS or media-content claim:

```sh
source scripts/env.sh
umask 077
./target/debug/live-production-supervisor --timeout-ms 120000 --grace-ms 10000 \
  -- node var/recovery-run-qa.mjs
```

## Unresolved local software gate: finite calibration tail

The extended target invocation enabled the real rehearsal, HTTP recovery and
local synthetic-controller drivers. It returned **56 passed, 1 failed, 0 skipped**.
The failure was `synthetic controller recovers real SRT AV without making an OBS
claim`, at `proveStrictCalibration`, not the corrected Rust server. The original
suite result was observed in the tool output; no separate full-suite log file
was written. Retained bad media independently reproduces the strict decode error.

`var/recovery-local-driver.CNjaoh/strict-calibration.ts` is 199092 bytes and
TS-aligned, but its final AAC access unit is truncated. Inspection found 146
decoded video frames and 227 decoded audio frames; video-only decoding passed,
audio/combined strict decoding failed with `TYPE_FIL: Input buffer exhausted
before END element found` and FFmpeg error `-1094995529`. The final TS packet
starts AAC PID `0x0042` at offset 198904; the 170-byte final access unit lacks its
continuation, unlike the normal 348/349-byte units.

One bounded quiet isolation run passed, retained separately in
`var/recovery-isolation-20261105/driver.log` and
`var/recovery-local-driver.LfgOpg/strict-calibration.ts`. That calibration has 229
decoded audio frames and complete final access units. This is evidence of an
intermittent finite-sender/receiver drain problem, not permission to erase the
failure. Concurrency is a possible trigger, not a proven cause. No harness code,
assertion or strict decoder requirement was changed or waived.

Root and the existing harness verifier should resolve finite-sender EOS and
receiver drain ownership using the retained files. The local software gate
remains open even though controlled acceptance and the isolated rerun passed.
To reproduce the retained failure without launching a sender:

```sh
source scripts/env.sh
./target/debug/live-production-supervisor --timeout-ms 30000 --grace-ms 5000 \
  -- ffmpeg -v error -xerror \
  -i var/recovery-local-driver.CNjaoh/strict-calibration.ts -f null -
```

The extended test command used the existing environment contract:

```sh
source scripts/env.sh
export LIVE_PRODUCTION_REHEARSAL_DRIVER=1 LIVE_PRODUCTION_REHEARSAL_ROOT="$PWD"
export LIVE_PRODUCTION_TEST_SERVER="$PWD/target/debug/live-production"
export LIVE_PRODUCTION_TEST_SUPERVISOR="$PWD/target/debug/live-production-supervisor"
export LIVE_PRODUCTION_TEST_WEB_DIST="$PWD/web/dist"
export LIVE_PRODUCTION_SUPERVISOR="$PWD/target/debug/live-production-supervisor"
export LIVE_PRODUCTION_RECOVERY_DRIVER=1 LIVE_PRODUCTION_RECOVERY_HTTP_DRIVER=1
export LIVE_PRODUCTION_RECOVERY_ROOT="$PWD"
export LIVE_PRODUCTION_RECOVERY_SUPERVISOR="$PWD/target/debug/live-production-supervisor"
export LIVE_PRODUCTION_RECOVERY_SERVER="$PWD/target/debug/live-production"
export LIVE_PRODUCTION_RECOVERY_WEB_DIST="$PWD/web/dist"
export FFMPEG="$LIVE_PRODUCTION_SYSROOT/usr/bin/ffmpeg"
export FFPROBE="$LIVE_PRODUCTION_SYSROOT/usr/bin/ffprobe"
umask 077
./target/debug/live-production-supervisor --timeout-ms 300000 --grace-ms 15000 \
  -- node --test scripts/target/*.test.mjs scripts/target/harness/*.test.mjs \
  scripts/media-network-gates.test.mjs
```

## Native and external gates remain separate

- Local CMake, Qt6 Core/Network/Widgets, libobs and OBS frontend SDK are absent.
  Portable C++ tests are not a plugin SDK build or genuine OBS recovery proof.
- The same retained native verifier owns exact checkpoint reruns of
  `2a7fdf6b741057e46aae18804dc5801804da48fe` and
  `9a07ef4beda93505a4c0d50485bc3c5f9df5df62`. No new native receipt was consumed
  here; prior native provenance/owner-loss failures are not cleared by local
  success. Root must relay native findings and any required follow-up candidate.
- The native harness must compare Program scene/content across daemon restart,
  not require equality of engine-run-scoped raw revision strings. This is a
  verifier handoff caveat, not a competing harness patch.
- Physical laptop ICE/UDP/TURN, intended-editor import, capture/encoder hardware,
  provider and human rehearsal remain unobserved. No public broadcast occurred.
- Primary `/workspace/main` remains clean at
  `77389b4fc6071c48f918f0368b11d14d99797f51`; preserved checkpoint refs are
  unchanged. Studio app PID/start ticks `419353/36675901` and supervisor
  `419348/36675900` were rechecked unchanged. Its executable, served bundle,
  state, password and recordings were not replaced.

## Retained artifact identities

| Artifact | SHA-256 |
| --- | --- |
| `var/acceptance/latest.txt` | `3317024183b191a664260a361e88ed2ce1423a22a6f341e1939a004c8717a9f8` |
| `var/recovery-run-http.imeMjo/report.json` | `7c3ba6458d296fed93b245d51c2faabc1f3446b087503d9dd427af7ec45f68b7` |
| Bad `CNjaoh/strict-calibration.ts` | `130cc419f11937e60a41cb85e82739a142f3a3b1aa8fa926d12034d8296780cb` |
| Good `LfgOpg/strict-calibration.ts` | `83863279d73c340cfb0c5311c853adf801cf103112359d51218214e94eda3619` |
| `var/recovery-isolation-20261105/driver.log` | `ab72ea97cce92450bff8c744c9c0a8890be6b6cfd3a003ebc878c0df5a8706a6` |
| `var/portable-cpp-verification/portable-cpp-summary.log` | `3c101c0ef7b5aa1bd92d59c4fff8d298d7e6a9d4f7e5d1263064b692ca667fc5` |
| `var/remaining-lifecycle-20261105/lifecycle.log` | `44925f074e86b55ccca229d356cb6d7f34c1fd6a40407d54025cca21e73c9835` |

The incremental Git bundle requires base `9a07ef4`; private evidence is delivered
separately, not tracked as production source. Its allowlisted archive excludes
state databases, passwords, source credentials and enrollment responses. Exact
final commit, bundle and archive identities are in the delivery receipt.
