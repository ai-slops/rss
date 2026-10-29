# Resumed native Ubuntu / OBS verification, 2026-10-29

## Coordinator decision

**Native dependency/build verification and the authenticated synthetic-media
workflow pass. Genuine OBS acceptance fails with a confirmed plugin null-pointer
crash. This is not a passing Ubuntu production candidate.**

The required next input is a reviewed fix from the existing OBS implementation
owner, relayed by the coordinator as an ordinary exact commit. Do not request
operator credentials, expand host capabilities, replace the VM, or implement a
second plugin. No protected merge, deployment, public broadcast, or existing
Studio operation is authorized by this receipt.

## Source and retained environment

- Fetched through the existing scoped Git origin and verified the exact primary
  checkpoint `22e7951757ddf3b0d85ed6d23363a2c2f33cee4c` from
  `work/target-rehearsal-20261028` before execution.
- Pushed the original `work/ubuntu-validation` branch normally at its unchanged
  `a4026e083548ee058a3a1dd2d0015528393198ea`.
- Separate integration worktree: `/workspace/ubuntu-obs-integration`, branch
  `work/ubuntu-obs-integration-20261028`. Merge `0059298` combines both histories;
  the original worktree/index and both original branch tips were preserved.
- Final executable verification checkout:
  **`9fa1395c136d6a62a9782f0f836d4747035e1f15`**. Guest `source-head.txt` records
  that value, `source-status.txt` is empty, and the exact primary checkpoint's
  ancestry was checked. Only documentation follows this tested code checkpoint.
- The same retained qcow2 overlay, backing image, client key and pinned SSH host
  key were reused. No replacement disk, VM, or credential set was created.
- Workspace `live-ubuntu-validation`, UUID
  `a73875ff-dba9-4785-83c1-5ab32e6d6258`, host `remote-main-4`.
- Guest boot ID throughout resumed execution:
  `51a87035-cff8-4849-a36d-5063e4cd18ab`.
- KVM/q35/host CPU, 4 vCPU, 8192 MiB configured; private userspace NAT and only
  loopback SSH `127.0.0.1:22428`. No host directory or Docker socket was mounted.
- No DIM APT mirror is selected. Native dependencies came from public Ubuntu
  repositories. Root/APT/GDB use was inside this one dedicated guest only.
- Retained explore and Oracle sessions were inspected and reused with their
  configured role models. No model/provider/auth configuration was changed.
  No cross-workspace HTTP authentication was attempted.

### Actual versions

Ubuntu **24.04.5 LTS**, kernel **6.8.0-142-generic**; GNU native toolchain,
Rust/Cargo **1.91.1**, GCC **13.3.0**, Node **24.18.1**, npm **11.16.0**,
GStreamer **1.24.2**, FFmpeg **6.1.1-3ubuntu5**, Qt **6.4.2**,
OBS **30.0.2.1-3build1**, libobs pkg-config **30.0.2.1**,
rswebrtc **0.14.3-RELEASE**. OBS uses Mesa llvmpipe software rendering.

## Ubuntu-side fixes and reproducibility

1. `scripts/ubuntu-vm.sh resume` checks the retained backing path/hash and
   existing disk/key/seed files, uses the existing ownership lock, and never
   regenerates them. Per-run logs preserve the first run. A bounded managed
   `ubuntu-verifier` service supervised this resumed runner; it has been removed.
2. `scripts/ubuntu-dependencies.sh` supplies native Qt6, OBS frontend development,
   GStreamer/nice/Pango, FFmpeg, and build packages. Ubuntu's `gstreamer1.0-x`
   supplies `timeoverlay`; preflight now checks it rather than discovering the
   omission after a synthetic sender exits.
3. Node `24.18.1` archive SHA-256 is pinned to
   `d6c664df3f3f61458e8c277585571328522d705166723a7c7823a9253a4d15a0`.
   The destination is recreated and its executable version asserted. A planted
   stale-file regression was actually removed during the final dependency run.
4. The published `gst-plugin-webrtc 0.14.3` crate archive is pinned to
   `3e8b92068d633cdb6be65196f45d082e7beaf5aba1a00c825753a038fac2dfc6`.
   Each build extracts that verified archive into a fresh directory outside the
   product Cargo workspace and uses a clean target directory, the included
   `Cargo.lock`, Rust `1.91.1`, and two build jobs. Cargo download caches are
   reused, not stale source/build products. The lockfile hash is
   `d6b1bd3cf31641e0bb290afad434e22c15016591892112884bf70542664f51d3`.
5. `scripts/ubuntu-env.sh` explicitly selects native mode and
   `RUSTUP_TOOLCHAIN=1.91.1`, project-local Node and the native rswebrtc plugin.
   Dependency setup asserts that `gst-inspect` actually loads the project-local
   plugin. No Alpine sysroot is used. The old fresh-check entrypoint now invokes
   this same dependency path and records the clean guest source.
6. CMake now links Ubuntu 30.0.2's actual exported target,
   `OBS::obs-frontend-api`; `OBS::frontend-api` was not exported by that package.
7. Native synthetic SRT senders use `mpegtsmux alignment=7`, keeping messages at
   1316 bytes instead of exceeding Ubuntu's `srtsrc` 4096-byte receive buffer.
   The network fixture uses finite sources and a clocked 500 ms drain branch
   before pipeline teardown. Receiver success and all **150 decoded video
   frames** are required, in addition to unchanged full `ffmpeg -xerror` decode.
8. The published OBS harness now monitors the supervised OBS exit while waiting
   for its report. It reports exit **139** promptly instead of waiting minutes
   for a report from a crashed process. New tests first failed with the old
   behavior, then passed; an already-written report still takes precedence.

The retained Oracle review approved the packetization/drain, VM ownership,
CMake and fail-fast changes. Its dependency-cache/fresh-flow findings were
corrected before the final native run. No blanket product-readiness approval is
claimed; the coordinator's independent pre-merge review remains required.

APT packages are version-inventoried, not repository-snapshot-pinned. The Rust
bootstrap script used by the fresh checker is downloaded and its observed hash
recorded, not pre-pinned. The toolchain and the Node/rswebrtc archives are pinned.
No second fresh VM was created to test that flow; it was exercised in the same
retained Ubuntu guest with freshly extracted dependency/build inputs.

## Commands and observed outcomes

Build/setup entrypoints, all inside the guest:

```sh
bash scripts/ubuntu-dependencies.sh
source scripts/ubuntu-env.sh
cargo build --locked --release --workspace
npm --prefix web ci
npm --prefix web run build
cmake -S integrations/obs-live-production \
  -B integrations/obs-live-production/build -DCMAKE_BUILD_TYPE=RelWithDebInfo
cmake --build integrations/obs-live-production/build --parallel 2
ctest --test-dir integrations/obs-live-production/build --output-on-failure
cmake --install integrations/obs-live-production/build \
  --prefix "$PWD/integrations/obs-live-production/package"
bash scripts/ubuntu-native-check.sh
```

Native release build, plugin build/staging, strict Clippy and formatting pass.
CTest passes **1/1**. The final native checker records:

| Gate | Exit | Result |
|---|---:|---|
| Locked Rust workspace build / fmt / strict all-target Clippy | 0 each | Pass |
| Cargo workspace tests | 101 | Provenance integration test fails; later test targets not claimed |
| Native lifecycle | 1 | Readiness fixed; owner-loss reaping assertion still fails |
| Stage A | 0 | Clean/Program full decode; **250.000 ms** calibration |
| SRT + loopback RTMP media network | 0 | Full strict decode, **150 SRT video frames** |
| Locked npm install / UI checks / 33 tests / production build / audit | 0 each | Pass; existing bundle-size warning remains |
| Bounded software OBS startup | expected timeout 124, classified pass | Real OBS initialization, not plugin interoperability |

Native driver verification used the repository supervisor with a 300-second
deadline and these environment variables:

```sh
export LIVE_PRODUCTION_REHEARSAL_DRIVER=1
export LIVE_PRODUCTION_REHEARSAL_ROOT="$PWD"
export LIVE_PRODUCTION_TEST_SERVER="$PWD/target/release/live-production"
export LIVE_PRODUCTION_TEST_SUPERVISOR="$PWD/target/release/live-production-supervisor"
export LIVE_PRODUCTION_TEST_WEB_DIST="$PWD/web/dist"
export LIVE_PRODUCTION_SUPERVISOR="$LIVE_PRODUCTION_TEST_SUPERVISOR"
export FFMPEG=/usr/bin/ffmpeg FFPROBE=/usr/bin/ffprobe
"$LIVE_PRODUCTION_TEST_SUPERVISOR" --timeout-ms 300000 --grace-ms 15000 -- \
  node --test scripts/target/harness/*.test.mjs scripts/target/*.test.mjs
```

**29 passed, 0 failed, 0 skipped** on the final tested checkout. This includes
genuine authenticated **GStreamer synthetic** SRT, public HTTP command workflow,
decoded color/tone evidence, 27 clean/Program segments, four crash fragments
with at least two confirmed closed, preserved raw hashes, stopped restored
outputs, and 31 export inventory entries. Portable package integrity and full
decoder checks pass; the application-import result remains
`notRun/targetEditorUnspecified`. This is explicitly **not OBS sender evidence**.

The SRT regression observed **147/150 frames** and failure with
`SRT_DRAIN_NS=0`, versus **150/150**, strict decode success and natural receiver
exit with `500000000`. Three enabled-drain runs passed before the final checker
also passed. No decoder threshold was weakened and no damaged tail was trimmed.

## Confirmed primary-owned OBS blocker

The documented `scripts/target/rehearsal.sh` was exercised with the staged
module, native release binaries, `/usr/bin/obs`, a fresh private evidence root,
and software GL/Xvfb. The final invocation used `--deadline-seconds 60` and
failed with:

```text
Native preflight: pass
[obs-live-production] module loaded
obs exited before its report: exit=139 signal=null
```

A separate supervised GDB reproduction against the same primary plugin source
confirmed **SIGSEGV**. The selected `Publisher::state()` frame had
**`this = 0x0`**. The nonsecret stack excerpt is:

```text
pthread_mutex_lock
Publisher::state()                         publisher.cpp:133
FixtureController::evaluate()
QMetaMethod::invoke / QMetaObject::invokeMethod
Publisher::initialize()
FixtureController::start_negative_attempts()
FixtureController::evaluate()              fixture.cpp:88
```

Relevant source: `fixture.cpp:88` starts negative attempts; `fixture.cpp:95`
dereferences each negative publisher. `fixture_prepare.cpp:117` sets
`negatives_started_` before constructing and initializing all publishers.
`Publisher::initialize()` state callbacks can synchronously re-enter evaluation
through the Qt UI task path. The observed nested evaluation encounters a still
null negative publisher. The plugin implementation files are unchanged from
the exact primary checkpoint; only its CMake target name was corrected here.

**Coordinator request:** have the existing implementation owner fix reentrant
negative-sender initialization/evaluation and supply an exact ordinary commit.
Protect partially constructed state without manufacturing bad-auth/duplicate
rejection evidence. Then rerun this same native harness and retained VM.

No successful OBS receiver-content, negative-admission, clean/Program recording,
recovery or export result is claimed. The harness never passes its fixture-report
gate. Three sampled post-crash snapshots showed both sources still connecting
with zero buffers. Normal software startup and module loading do not establish
authenticated media interoperability. The Tools dialog was not manually
accepted through this crashing fixture path.

## Each previous blocker

| Earlier failure | Current disposition |
|---|---|
| Missing `webrtcsink` | **Fixed and exercised**, native pinned `0.14.3-RELEASE`; preflight and daemon startup pass |
| SRT H.264 `0x0` / missing PPS / decode 69 | **Fixed for the native network fixture** by bounded packetization and drain; exact-frame and strict decode regression passes |
| Clean video provenance loss | **Still failed.** With valid packetization the test now fails at `server/tests/editor_clean_provenance.rs:85`, observed generations `{1}` versus required `{1,2}`; the final run reports double-free/corruption and SIGABRT during unwind. Earlier SIGSEGV is retained. No assertion was weakened |
| Lifecycle before-readiness exit | Missing-rswebrtc cause confirmed from retained server log and **readiness fixed**. Ten native outcomes restore baseline, but owner-loss still fails `native-lifecycle.test.mjs:41` because the PID remains observable where ESRCH is required. Seven process-tree cases pass; full lifecycle does not |
| Native OBS plugin untested | Native build/link/load **verified**; fixture execution **fails with confirmed null-Publisher SIGSEGV** and needs its existing owner |

The provenance/reconnect/unwind and owner-loss behavior remain shared
application/supervisor dependencies for the coordinator, not successes or
permission to rewrite those components in this verification track.

## Artifact integrity and runtime disposition

Private evidence is retained under `/workspace/main/var/ubuntu-validation/` and
inside `/home/validator/integration/var/` on the stopped guest. Raw archives
include private fixture state and scoped credentials: **do not publish or attach
them to a public review**. Only this sanitized receipt and source tooling are
committed/pushed. Original first-run evidence and both failure archives remain.

| Private archive | SHA-256 |
|---|---|
| `ubuntu-integration-evidence-20261029.tar.gz` | `b700e5fe567ed565f5a5b66cab6a4eb3c6d8bc9784f7d02d1a4c49fa49b718ea` |
| `ubuntu-reviewed-evidence-20261029.tar.gz` | `069216bbf56c81a259e3449674da8a7b9300c12aaf74ff9a7bff375726e899ee` |

GDB null-pointer evidence: guest
`var/rehearsal-06/private/obs-gdb-null.txt`. Final public-surface failure:
`var/reviewed-native/rehearsal.log`. Source attestation, target tests, binary
hashes and versions: `var/reviewed-native/`. Native checker: `var/ubuntu-native/`.

Final tested binary hashes:

```text
obs-live-production.so    752e7449b07303b6daec07112ca20e14f5094ec8f8ee58693e579fed451db60c
live-production          eb3a83b206d82f66440295553731541b650bbb9709512592c715abf3f715bf12
supervisor               283494ab2251f44efec01a23717168ca0c7fa50ab2458f3598acb274438b9dac
libgstrswebrtc.so         70b808fd3e0a1bd9b17911a9d7f55805092f16d1a9da151485299e3c88d37991
```

The resumed VM ran from `2026-10-29T03:18:50Z` until
**`2026-10-29T04:59:32Z`**, exact QEMU PID **4399**, then stopped through its
managed ownership path. PID 4399 is absent, the `ubuntu-verifier` registration
was removed, and the workspace service list is empty. The runner's final 143 is
the requested managed stop, not a passing application test. No OBS/Xvfb/GDB or
product process remained in the guest before shutdown. The original backing
SHA-256 still matches
`6a81c37564db9b1ee84e141922625e1d7c5b389b99bb3c572e0243607d5bb4d2`.
The one private overlay, identity and caches remain available for the next exact
primary fix; no automatic boot or test retry is scheduled.

Shell syntax/diff checks and Node tests passed. LSP could not inspect this
separate worktree because the tool restricts paths to the original request cwd;
no clean LSP result is claimed. Compiler/Clippy/native execution supply the
available static/runtime evidence.

Still external: primary-owner fixes above, longer native OBS soak/fault profile,
target laptop OS/browser and external-network ICE, physical capture/hardware
encoding, intended editor choice and real application import, and platform
broadcast semantics. No public transmission was started.
