# Root relay: fixed native EOS comparison results

## Disposition remains partial

The four predeclared cases all passed. **The original 196.93-second EOS failure
was not reproduced, its cause/loss point is still unknown, and it is not cleared.**
No production correction was made, no failed case was retried into a pass and
no EOS/decode/duration/fragment/rate/RSS assertion was relaxed. The real OBS30
publisher-only freeze remains unavailable until primary supplies and root relays
a compatible production seam. No native acceptance claim is made here.

## Exact execution identity

- Executed clean source: **`c85b56ae75243c69a8478f4d8a183e9218d853ae`**.
- Production trees `server/src`, `media/src`, `integrations/` remain byte-identical
  to `de4b80058871f4ec5f138ddb25cb68d05c5bc718`; checked at guest entry.
- Plan committed before execution at `0b0b0b3`:
  `docs/eos-comparison-plan-2026-11-06.md`.
- Same retained KVM overlay, Ubuntu 24.04.5/GStreamer 1.24.2/Rust 1.91.1,
  4 vCPU/8192 MiB; no new VM or workspace. Existing verifier/dirty worktrees,
  old checkpoints and previous media remain untouched.
- All four cases use the exact original named soak, with only opt-in test-boundary
  observations. No bus consumer or pad/event injection was added. The public
  output snapshot observer records Running/Finalizing/Stopped and the Stop result.

Owned host command:

```sh
LIVE_UBUNTU_STATE=/workspace/main/var/ubuntu-validation \
  timeout --signal=TERM --kill-after=60 1800 \
  bash scripts/ubuntu-vm.sh resume bash scripts/ubuntu-eos-matrix-host.sh
```

Guest precompiled the test, then supervised this fixed matrix with a 1450-second
overall bound and a 300-second bound per case:

```sh
target/debug/live-production-supervisor --timeout-ms 1450000 --grace-ms 10000 -- \
  node scripts/target/eos-soak-matrix.mjs \
  "$PWD/target/debug/live-production-supervisor" "$PWD/var/de4-eos-matrix/cases"
```

Each case executed:

```sh
cargo test --locked -p live-production-media --test srt_drift_endurance -- \
  --exact sustained_independent_clocks_report_drift_and_preserve_bounded_progress \
  --nocapture
```

Cases ran once in their declared order. Minimal cases have no `GST_DEBUG`
override. Traced cases use
`splitmuxsink:5,matroskamux:4,GST_EVENT:5,GST_BUS:5,GST_STATES:4,queue:5`.
Each loaded case has exactly one separately owned finite busy-loop Node worker;
the worker is stopped before independent raw-media inspection. Private per-case
`TMPDIR` and unwind retention preserve artifacts without changing output policy.

## Measured case outcomes

| Case | Soak exit | Test duration | Original Program frames | Stop API call | RSS growth KiB | Raw strict decode |
|---|---:|---:|---:|---:|---:|---|
| idle-minimal | 0 | 182 s | 5409 | 1403 ms | 186360 | 37/37 pass |
| idle-traced | 0 | 182 s | 5408 | 2005 ms | 216880 | 37/37 pass |
| loaded-minimal | 0 | 182 s | 5408 | 1468 ms | 202532 | 37/37 pass |
| loaded-traced | 0 | 183 s | 5409 | 2416 ms | 217276 | 37/37 pass |

Matrix exit 0 is a **diagnostic result**, not completion of native acceptance.
All 148 inspected raw fragments have retained SHA-256, FFprobe frame/stream data
and successful `ffmpeg -v error -xerror ... -f null -` results. Duplicate evidence
copies are not counted twice. Original media content checks also pass; PCM RMS
is 2789.31, 2786.05, 2788.98 and 2816.30 respectively. No raw failed fragment was
generated in these four executions; this is not evidence about the missing raw
file from the first failure.

Stop API timing is measured around the original `outputs.stop()` call, including
its existing final catalog/validation work. Observer lifecycle takes up to one
additional 200 ms sampling interval; it is reported separately and does not
extend production's unchanged 15-second EOS deadline. All cases share the optional
observer, so even the minimal cases are not a claim of an uninstrumented binary.
The observer and logging may perturb scheduling. Four passes cannot establish
that logging caused or fixed the historical failure, or rule out a race.

## Observed EOS/drain timeline in the two traced cases

These are GStreamer relative process timestamps, not wall time or inferred PTS.
Both traces identify the recorder as `recording_mux` in `pipeline1`, bus `bus12`.

| Event | idle-traced | loaded-traced |
|---|---|---|
| Recorder reference EOS | 180.763907112 s | 180.781538505 s |
| Audio queue EOS | 180.767677105 s | 180.785305565 s |
| Video queue EOS | 180.767699158 s | 180.785334711 s |
| Final fragment 36 closed | 180.768467532 s | 180.786149989 s |
| Recorder pipeline EOS consumed on bus12 | 180.768553897 s | 180.786255459 s |
| Recorder pipeline completed Null | 180.773709168 s | 180.793382993 s |

Each trace has 37 fragment-open and 37 fragment-close events. The final fragment
closes before the recorder pipeline EOS is consumed. Reference-EOS-to-bus-EOS
is approximately 4.65 / 4.72 ms; the additional Stop duration is not evidence of
a 1.4–2.4-second EOS wait. Catalog/validation work is part of the same API call.
There is no observed EOS loss point in these successful traces.

Concrete private references:

- `idle-traced/test.log:433649`: reference EOS.
- `idle-traced/test.log:433979`: audio queue EOS; line 433981 video queue EOS.
- `idle-traced/test.log:434046`: final fragment close.
- `idle-traced/test.log:434061`: recorder bus EOS consumed.
- `idle-traced/test.log:434739`: recorder Null complete.
- `loaded-traced/test.log:433486`: reference EOS.
- `loaded-traced/test.log:433818`: audio queue EOS; line 433820 video queue EOS.
- `loaded-traced/test.log:433885`: final fragment close.
- `loaded-traced/test.log:433897`: recorder bus EOS consumed.
- `loaded-traced/test.log:434577`: recorder Null complete.

The summarizer's 619 `busEosCandidates` per traced case includes many decode/
validation pipelines and cannot be interpreted as recorder EOS proof. The
pipeline-specific correlation above comes from the actual bus/mux/state trace.
Minimal-case internal event counts of zero mean tracing was disabled, not that
EOS was absent. Their Stop returned `Ok`, state reached Stopped and strict media
and all original assertions passed.

## What primary still needs to distinguish

Production `media/src/output_runtime_stop.rs` accepts pipeline EOS, then waits
for EOS **or Error** with `timed_pop_filtered`. It currently emits the same
`recording pipeline did not finalize with EOS` error for a non-EOS message and
for no message. The first failure's short log does not disambiguate those paths.
It remains possible that a future reproduced failure exposes rejected upstream
EOS, one undrained branch, asynchronous mux finalization, or a bus Error. No one
of these hypotheses is asserted as the cause. The first file/event sequence
cannot be reconstructed from these passing files.

The verifier has not added temporary production logs or modified Stop ordering.
These test-only boundary/native trace tools and retention are available for the
next focused candidate. Primary owns any production fix and its precise failure
reason/ownership instrumentation. Do not clear the first failure merely because
this fixed comparison did not reproduce it.

## Freeze coverage preparation and regressions

`docs/publisher-freeze-native-proof-plan-2026-11-06.md` records the backend-neutral
adapter boundary and real native cases: controller continuously alive, actual
receiver quiescence, finite resume and true recreation/new generation, sustained
strict recovered media, stale-epoch resume fencing, duplicate idempotence and
manual Stop cleanup. It deliberately does not invent primary's next interface
or remove the current unavailable flag. No source mute, receiver bridge fault,
thread suspension or whole OBS stop may satisfy this independent transport case.

The prior genuine OBS30 audio/tone, both per-track `{1,2}` provenance, 12 CTests,
full-flow safety/recovered-A/V and Program-fence passes remain separate measured
regressions. They were not rerun or relabelled in this EOS-only slice. The prior
target suite's optional export-package fixture remains explicitly not executed.
Physical/editor/provider gates remain separate and do not excuse software faults.

## Validation, privacy and stopped runtime

- New observer tests: 2 pass, including original error preservation and monitor
  release during a deliberately caught test unwind.
- New event evidence regression: 1 pass, locally and on native Ubuntu.
- Native compilation, formatting and strict workspace/all-target Clippy: pass.
- LSP remains path-restricted outside the original request cwd; no clean LSP claim.
- Guest source status is empty. Post-run process inventory has no owned load,
  cargo/test, media, OBS or supervisor processes left.
- Final QEMU PID **27375** stopped **2026-11-06T16:02:32Z**, runner exit 0. No
  automatic retry or boot is scheduled. No new production checkpoint was present
  in the authorized delivery directory when checked after the matrix.
- No protected/main merge, push, publication, deployment, force operation,
  provider broadcast, Studio, operator network, account or credential change.

Private archive:
`/workspace/main/var/ubuntu-validation/eos-native-evidence-01-20261106.tar.gz`.
SHA-256 **`0cd9db64b0505c3a8908522849401a8c338c3a9a67c9aa0a0e10e5d21f47f117`**.
Extracted ignored root: `var/de4-eos-matrix/`; `cases/matrix.json` is the fixed
plan/outcome ledger, and each case has `test.log`, `events.json`, `raw-media.json`,
media/session copies and its private temporary directory. Do not publish raw
archives or native logs. Later receipt-only commits are not additional execution.
