# Retained Ubuntu replay of production de4

## Disposition: partial, not native acceptance

The final complete-flow receipt is **partial**, process exit **1**,
`nativeExecution:true`, `obsClaim:false`, `diagnosticOmissions:[]`, with exactly
one `requiredUnavailable` entry: **`publisherTransportFreeze`**. Every other
assertion in that full flow executed successfully. A separate earlier soak
failure remains unresolved; its later passing replay does not erase it.

Root owns the production corrections, final integration and publication. No
protected/main merge, push, force operation, deployment or public/provider
broadcast occurred. All execution used the same retained VM/qcow2 overlay and
pinned SSH identity. The new directory is an ordinary integration worktree in
the existing workspace, not a replacement workspace or VM.

## Exact ownership and source identity

- Delivered production: `de4b80058871f4ec5f138ddb25cb68d05c5bc718`.
- Verified incoming bundle SHA-256:
  `0951cf6ee8b64e865837aba5e9df6c5da772f4c763ee6ae2577d735f5845519b`.
- Bundle verification confirmed required base
  `9a07ef4beda93505a4c0d50485bc3c5f9df5df62` and exact named branch. It had no
  `HEAD` ref; the first attempted HEAD fetch failed harmlessly, then the exact
  named branch was imported as `import/production-de4b8005`.
- Preserved verifier: `e6afd84b26c428696dc922b5c489b6ddcc1136f5` plus its untouched
  dirty overlay in `/workspace/ubuntu-recovery-review`.
- Tracked overlay patch SHA-256:
  `0a4b46e188de419571191744d161b5000e07eeece4dfdc0e6e18820da184104c`.
- Untracked overlay archive SHA-256:
  `dd37abf3a0a71bfa03e36ff0bddf1a3664bf31fbc3bf14ffe289351d6a0657b9`.
  Both are private under `/workspace/main/var/ubuntu-validation/`, named
  `verifier-before-de4.patch` and `verifier-before-de4-untracked.tar.gz`.
- Integration worktree `/workspace/ubuntu-de4-acceptance`, branch
  `work/ubuntu-de4-acceptance-20261106`; conflict-free integration merge `18c471c`.
- **Final executed clean integration SHA:
  `7709ded2d4e4d7e9a5969f444eee104e15f14040`.** Guest status was empty at entry
  and exit. Later receipt-only commits are not represented as another execution.
- At every native gate entry, `server/src`, `media/src` and `integrations/`
  were checked byte-identical to exact production de4. Verifier changes affect
  only test fixtures, evidence retention, harness assertions and owned runners.

## Native environment and bounded commands

Ubuntu 24.04.5, kernel 6.8.0-147, native OBS/libobs 30.0.2.1, Qt 6.4.2,
GStreamer 1.24.2, FFmpeg 6.1.1, Rust/Cargo 1.91.1 and Node 24.18.1. Retained
KVM/q35/host CPU VM, 4 vCPU/8192 MiB. No physical capture or hardware encoder
is assumed. Build cache reuse affected only the stopped verifier checkout;
there was no concurrent guest Studio. The web dependencies were installed with
the existing lockfile in the new checkout, not in an operator installation.

Owned host execution used:

```sh
LIVE_UBUNTU_STATE=/workspace/main/var/ubuntu-validation \
  timeout --signal=TERM --kill-after=60 2400 \
  bash scripts/ubuntu-vm.sh resume bash scripts/ubuntu-de4-host.sh
```

Subsequent bounded host workloads were `ubuntu-de4-replay-host.sh`,
`ubuntu-de4-final-host.sh` and `ubuntu-de4-complete-host.sh` under that same
VM owner, with fresh numbered private evidence roots. Their exact historical
versions and guest source hashes are retained. Last workload limit was 600 s.

Principal guest commands, after `source scripts/ubuntu-env.sh`:

```sh
cargo build --locked --workspace
cargo build --locked --release --workspace
npm --prefix web ci
npm --prefix web run build
cmake -S integrations/obs-live-production -B var/de4-plugin-build \
  -DCMAKE_BUILD_TYPE=RelWithDebInfo
cmake --build var/de4-plugin-build --parallel 2
QT_QPA_PLATFORM=offscreen timeout 180 \
  ctest --test-dir var/de4-plugin-build --output-on-failure
cmake --install var/de4-plugin-build --prefix "$PWD/var/de4-plugin-stage"
```

Both genuine OBS entrypoints used that newly built plugin and the locked release
server/supervisor, `/usr/bin/obs`, private Xvfb/loopback SRT and fresh evidence:

```sh
scripts/target/rehearsal.sh --plugin-dir "$PWD/var/de4-plugin-stage" \
  --evidence-dir NEW --server-binary "$PWD/target/release/live-production" \
  --supervisor-binary "$PWD/target/release/live-production-supervisor" \
  --obs-binary /usr/bin/obs --deadline-seconds 120

scripts/target/recovery-native.sh --plugin-dir "$PWD/var/de4-plugin-stage" \
  --evidence-dir NEW --server-binary "$PWD/target/release/live-production" \
  --supervisor-binary "$PWD/target/release/live-production-supervisor" \
  --web-dist "$PWD/web/dist" --obs-binary /usr/bin/obs \
  --xvfb-binary /usr/bin/Xvfb --deadline-seconds 300
```

Recovery safety runs separately set
`LIVE_PRODUCTION_RECOVERY_DIAGNOSTIC_MODE=fences|budget|controls`. They intentionally
return partial/exit 1 even when every named case passes; they are not full native
acceptance. No diagnostic omission was used in the final full flow.

## Actual results

| Gate | Measured result |
|---|---|
| Locked debug/release, plugin build/install | Pass |
| Native CTest | 12/12 pass, including real libobs two-mix audio and pidfd tests |
| Genuine OBS30 rehearsal | Pass; bad-auth and duplicate publishers rejected |
| Rehearsal Program | 9 strictly decoded H.264/AAC segments, red-left/blue-right, both tones, RMS 0.222109 |
| Rehearsal red/blue cleans | 8 segments each, changing video; RMS 0.248257 / 0.249040, correct 440/880 Hz dominance |
| Original editor clean reconnect provenance | Pass; generations `{1,2}` required independently on each track; catalog reload included |
| Black/silent metadata-free provenance | Pass, each track `{1,2}` despite identical content |
| Bounded Program / causal capture / lag contracts | 3/3 pass, strict fragments and common origin retained |
| First original 180-second soak | Fail, exit 101 after 196.93 s: recorder did not finalize with EOS |
| Instrumented original soak replay | Pass, 181 s, 5409 Program frames, 37 strict segments, RSS growth 223676 KiB, PCM RMS 2789.24 |
| Output lifecycle filter | 5 passed; startup/failure/Stop cleanup assertions retained |
| Recovery unit and HTTP tests | 13 + 1 passed |
| Live engine-run fence driver | Pass; old pending cancellation, exact old duplicate/late ACK rejection, budget 1→1, real new-run generations 1/2 and unchanged Program |
| Final full genuine OBS flow | Partial/exit 1 solely for unavailable publisher-only freeze; all other full-flow assertions pass |
| Native `fences` mode | All 10 cases pass, including strict initial media, 401s, stale process/poll/intent 409s, exact/changed duplicates, token rotation, queued Stop and fresh OBS cannot arm |
| Native `budget` mode | All 4 cases pass: strict media, 1/2/4 s backoff, exactly 3 executions, concurrent idempotent retry, queued Stop |
| Native `controls` mode | All 6 cases pass: strict media, reconnect/lease, maintenance, disable, restart/action fences |
| Final target tests | Exit 0: 70 pass, 0 fail, 1 skip out of 71; optional real-package export fixture test had no supplied fixture |
| Web tests / build | Pass after correcting the verifier-only dependency setup |
| Formatting / strict all-target Clippy | Pass on final executed integration |
| LSP | Not available: tool rejects paths outside original request cwd; no clean LSP claim |

The first soak's temporary directory was removed during unwind before its old
post-success preservation call. This raw-media evidence limitation is explicit.
An unwind-safe test-only guard now retains subsequent failed/successful soak and
new causal-recording directories. The passing replay had no production change;
GST mux diagnostics and extra pre-finalization observations changed scheduling.
The initial EOS failure is **not cleared** or silently retried into acceptance.

## Final full-flow observations

- Bridge DATA fault: controller online; real publisher restart; receiver
  generation 1→2, advancing video/audio/PTS; 4 strict recovered clean segments,
  3.965643 s, RMS **0.246536**.
- Duplicate action: same completed outcome, no additional execution.
- Separate receiver DATA freeze: generation 2→3. It remains explicitly
  `independentFaultMechanism:false`; it is not publisher-freeze proof.
- Whole OBS process suspension: controller offline then online and usable A/V
  in generation 4; not a process-restart capability claim.
- Real daemon restart: new engine run; startup preserves retry budget and no stale
  action executes. Actual OBS media recovers, then yields 4 strict clean segments,
  4.03606 s, RMS **0.248986**, H.264 1280x720 plus AAC stereo 48 kHz.
- Maintenance/source-disable suppress actions; re-enabled listener recovers.
- Three forced retry failures exhaust the budget. A fresh manual retry queues
  a nonempty command; manual Stop cancels it, dispatcher resume executes nothing,
  and receiver counters/PTS become quiescent.
- Output count and actual Program content remain unchanged. Run IDs are equal
  inside each intentional restart interval; Program revision equality is enforced
  there. Across the explicit restart, actual scene/layout/canvas/assets/audio
  routes are compared rather than requiring the old revision token.

Owned harness fixes retain every original assertion: fresh peer handover is now
explicitly authorized/revoked at OBS resume, daemon restart and listener
re-enable. Previously rejected peer counts and all failed runs remain retained.
Synthetic-controller recording now maintains its actual controller lease rather
than timing out a new strict post-capture progress check. Strict recording also
requires at least 3 s aggregate decoded media. Oracle's identified freeze-proof
and Program-fence gaps were corrected before the final executions.

## Primary action required: genuine publisher-only freeze

The actual `freezePublisher` command returned **`publisherTransportNotIdentified`**
while the verified OBS executable had **no direct children and no mux candidate**.
This is observed native rejection, not absence inferred from acknowledgement
alone. Repeated private receipts contain the exact command/result and process
inventory. No usable-media claim is made for this unavailable case.

Official OBS30.0.2 source at commit
`afbebf94867982b7c294ebc1b8c04438f8b59536` explains the incompatibility:

- `plugins/obs-ffmpeg/CMakeLists.txt` defaults `ENABLE_NEW_MPEGTS_OUTPUT=ON`.
- `obs-ffmpeg-mpegts.c` registers `ffmpeg_mpegts_muxer`, opens libsrt/AVIO and
  writes from an in-process pthread.
- The `obs-ffmpeg-mux` subprocess is in the separate legacy
  `obs-ffmpeg-mux.c` implementation, not this native output.

Primary must supply a bounded, epoch-fenced, truly per-publisher transport fault
compatible with this actual OBS30 output, or another explicitly reviewed genuine
publisher implementation. The verifier did not change production, replace OBS,
pause arbitrary threads/the whole process or substitute bridge DATA drops.
Root should also retain the EOS failure as unresolved and request primary's
targeted investigation rather than promoting its later pass.

## Private evidence and stopped runtime

Private archives in `/workspace/main/var/ubuntu-validation/` and extracted
`var/de4-native*` are ignored, contain credentials/state/media and must not be
published. Code-only bundles contain only tracked source/docs, not those files.

| Private archive | SHA-256 |
|---|---|
| `de4-native-evidence-02.tar.gz` | `00a55e71fca41e4080fd81d12f6ef22dcde205ac06b9abb2bbb80b96af1b02db` |
| `de4-native-evidence-03.tar.gz` | `f7bd0b6a4c232c07f23b3c504248340375849fc3bc1eb846c6585acc77f5ace5` |
| `de4-native-evidence-04.tar.gz` | `54febd64ccea42def537873ab266415ea7a4163d13e5889dff2122fe467b6f9e` |
| `de4-native-evidence-05.tar.gz` | `639857d479f4f0de7c0be3ccab4615e3e6bd3523545b293174f36ed0b5a9e172` |
| `de4-native-evidence-06.tar.gz` | `24dd986465a79b46f193f6aa8199e5a14df087ce21c7319995353279951bd01d` |
| `de4-native-evidence-07.tar.gz` | `99be306381c7243348b2efc1c40e1cf3890538f5b2c27a50e9ae6b0c9cd96e23` |

Final QEMU PID **25964**, safely stopped **2026-11-06T11:35:11Z**, runner exit 1.
No automatic retry/boot is scheduled. Final guest status is empty. Earlier dirty
meter/probe worktree and all original failed acceptance media remain untouched.

Physical target-laptop ICE, capture/GPU, actual intended editor and authorized
provider rehearsals remain separate external gates. The skipped optional export
fixture test is an unexecuted software check, not physical-editor proof. None of
these external limitations explains away the executable native freeze/EOS blockers.
