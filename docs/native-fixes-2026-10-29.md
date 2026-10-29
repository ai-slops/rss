# Focused native crash/lifecycle handoff

This ordinary candidate reconciles primary `1c8d8adffb8d1c48f8b7c2cc39e38e7009f1d7a0`
with verifier receipt `46eca92769dbdf47d9f79dc8ab555779a5513b8f`, whose tested code
was `9fa1395c136d6a62a9782f0f836d4747035e1f15`. Original checkpoints and the
running studio remain untouched. Root owns independent final review and protected
promotion. Nothing here authorizes deployment or public/platform output.

## Changes for prompt relay to the same verifier

1. **Real OBS null-publisher crash:** negative publishers are all constructed
   before initialization. A shared transition/evaluation state gate defers and
   coalesces synchronous callbacks. Initialization errors remain `fixtureError`,
   never admission rejection. Bad-auth/duplicate checks still require actual
   output signals and decoded receiver content.
2. **Queued callback lifetime:** fixture evaluation uses a dedicated Qt receiver.
   Queue admission and shutdown share a mutex; closing cancels only that receiver's
   pending work. No queued raw controller pointer, global MetaCall flush, or
   fixed-count teardown drain remains. Generation invalidation fences stale work.
3. **Owner-loss reaping:** the original ESRCH assertion remains. The fixture now
   acknowledges the exact PID/start-tick generation from Node's child `close`
   callback, after reaping. The prior native failure could observe baseline
   equality during a short zombie interval, because `/proc/PID/exe` is unreadable
   for zombies. No supervisor runtime or process-disappearance criterion was waived.
4. **Native corrections retained:** pinned native rswebrtc/Node setup, installed
   OBS frontend CMake target, Pango preflight, SRT alignment=7 and bounded drain,
   complete 150-frame validation, packetized authenticated fixture senders, and
   immediate OBS-exit reporting. The primary credential/CLI, stereo controls,
   strict H.264 headers, and provenance artifact preservation are also retained.

## Observed local checks

- The actual state gate's compiled C++17 `NDEBUG` regression passed, including
  synchronous partial-construction callbacks, initialization failure classification,
  stale generations, nested transitions, cleanup, and concurrent callback coalescing.
  The owner also executed optimized stress and UBSan runs.
- Parent executed `bash scripts/lifecycle-acceptance.sh`: all eight tests passed,
  covering eleven native-app outcomes and seven process-tree cases. Owner-loss
  matches PID/start ticks and still requires strict ESRCH; native baseline restored.
- Target/configuration suite: 34 passed, zero failed, three native-driver cases
  intentionally not enabled in this quick checkpoint run. Native synthetic tests
  passed in the retained verifier's earlier receipt, not against this fix yet.
- Full Qt dispatcher regression and real OBS fixture cannot execute in this
  ordinary Alpine workspace without Qt/libobs development/runtime dependencies.
  They are supplied as native CTest executables, not claimed passed here.
- The retained independent reviewer approved this focused checkpoint for native
  handoff, not product acceptance. Root's final independent review remains open.

## Exact next native actions

Use the **same** verifier session and retained VM. Do not replace the disk, key,
environment, or verifier. Fetch the candidate's exact ordinary SHA through the
coordinator, record actual source status, then:

```sh
source scripts/ubuntu-env.sh
cmake -S integrations/obs-live-production \
  -B integrations/obs-live-production/build -DCMAKE_BUILD_TYPE=RelWithDebInfo
cmake --build integrations/obs-live-production/build --parallel 2
ctest --test-dir integrations/obs-live-production/build --output-on-failure
cmake --install integrations/obs-live-production/build \
  --prefix "$PWD/integrations/obs-live-production/package"
bash scripts/lifecycle-acceptance.sh
scripts/target/rehearsal.sh \
  --plugin-dir "$PWD/integrations/obs-live-production/package" \
  --evidence-dir "$PWD/var/native-fixed-obs-new-run" \
  --server-binary "$PWD/target/release/live-production" \
  --supervisor-binary "$PWD/target/release/live-production-supervisor" \
  --obs-binary /usr/bin/obs
```

Rebuild application/supervisor binaries from the new SHA before running. Evidence
roots must be fresh. Return CTest results for both fixture state and dispatcher,
real OBS exit status/report, bad-auth/duplicate outcomes, actual decoded video and
audio, clean/Program recording and export results, and lifecycle owner-loss proof.
See `../integrations/obs-live-production/NATIVE_REENTRANCY_RERUN.md` for details.

## Provenance still open, not fabricated

The latest native receipt now reports generations `{1}` instead of `{1,2}` and
corruption during test unwind. This checkpoint does not claim a metadata fix.
Its unchanged assertions and finalized session/MKV preservation must run with
the now-packetized sender. Follow `../media/NATIVE_PROVENANCE_RERUN.md`, retain
the real final JSON, all MKVs, memory-identity output, source progress and failure
logs. Do not infer generations from pixels, GAP flags, arrival order, or current
connections. The previous content-hash workaround was rejected and is absent.

Source auto recovery is a separate subsequent phase. It must remain opt-in,
source-scoped, bounded, and unable to start public outputs, alter scenes, or
reverse an intentional Stop. This crash checkpoint does not enable it.
