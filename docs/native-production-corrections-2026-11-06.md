# Native failure corrections for same-verifier replay

## Checkpoint and authority

This production correction branch is based on
`88e0cd7ceefe3e3ce2608a9510c189612bab2402`; its persisted engine-run recovery
fences remain intact. Root owns independent integration and publication. No main
merge, push, force operation, deployment or public broadcast was performed.

The relayed code-only bundle's SHA-256 was verified as
`69584fe884d4bafa3c2ceb0da54706addcb914c8f8479cc6bfe9252110a9cf0e`.
It was imported into `review/ubuntu-native-completion-20261106` at
`b2c6e22f3fc42206803cf1c5a72bb75321131489` and a detached inspection worktree.
Its production trees were confirmed identical to `9a07ef4`. The receipt and
manifest for canonical native run `1228a8d4a612e2b959eff64ff8b3722f84233b08`
were read. No reverted fixture-clock/activation experiment was imported as a fix.

**Native acceptance is not declared passed.** The prior OBS 30/GStreamer 1.24
failures remain historical failures until the same retained verifier executes
this corrected checkpoint. The changes below address software defects, not
unavailable hardware. No verifier credentials, session or VM were accessed.

## Corrections

### Real OBS fixture audio roots

OBS 30's `audio_callback` renders all audio sources but mixes only `root_nodes`
reachable from main output channels. An auxiliary `obs_view` does not add an
audio root. This explains why video and active senders could coexist with
numerical AAC silence, and why timestamp/active-reference changes alone failed.

The private fixture now claims an unused output channel above channel 0 for
each generated source. It never replaces an existing source. Explicit teardown
removes only matching fixture pointers before releasing the sources, including
configuration failure, deadline and ordinary shutdown. Normal Publisher routing,
the selected global mix and fixture A/V timestamps are unchanged.

A real libobs/Xvfb/llvmpipe component regression starts both video and audio,
checks two separate mixes with absolute RMS and tone amplitude, and rejects
cross-mix leakage. Against the unchanged `88e0cd7` source it measures zero in
both mixes and exits 1. With the correction:

```text
mix 0 RMS=0.176742 tone amplitude=0.249902 other tone=0.00000192692
mix 1 RMS=0.176734 tone amplitude=0.249878 other tone=0.0000402748
```

This local SDK is Alpine OBS 31.1.2/Qt 6.10.3, isolated under `var/obs-audio-sdk`.
It is executable component evidence, not the retained Ubuntu OBS 30 acceptance.
The SDK's Xvfb required a private executable-path relocation helper for xkbcomp;
that helper affects only Xvfb startup, not libobs, audio data or assertions.

### Recorder timeline and error finalization

The real encoder regression observed an encoded video segment/PTS near
`999:59:59.996837309`, while audio uses a zero-based segment. Before splitmux,
`identity single-segment=true` now converts encoded video to running time. The
regression checks actual mux-pad segment start, PTS and running time, rather than
only an element name. It failed without rebasing and passes with it. This targets
the older splitmux timestamp handling implicated by the native short/empty files.

Recording startup now owns rollback from session creation onward. Pipeline,
clock or feeder-start failure forces Null, marks the session Failed and retains
recoverable raw files. Operator Stop cannot bypass cleanup on a feeder error.
Error-only teardown sends an out-of-band FlushStart before feeder EOS/Null;
healthy recordings retain the existing strict EOS result and 15-second deadline.
GStreamer failure details reach output state rather than only a generic error.

Empty and video-only failure regressions require bounded Null teardown, Failed
metadata, persisted provenance where available and non-complete raw recovery.
The prior timeout itself did not naturally deadlock on local GStreamer 1.26;
these tests establish abort ordering and cleanup, not a fabricated reproduction
of the native hang. No fragment was trimmed or promoted to decode-validated.

### Buffer-causal clean provenance

Clean recorders no longer rely on metadata surviving `intervideo`/`interaudio`.
At the existing post-epoch ingest pad, an engine-owned bounded registry captures
the exact buffer, caps, immutable source/track/run/generation stamp and sequence
as one packet. Clean recording appsrc feeders transport that packet directly.
Program and RTMPS keep their existing inter-tap paths.

The same track hubs survive source reconnect. Consumer reference metadata is
removed deliberately; provenance comes only from the packet's captured stamp
after appsrc acceptance. Byte-identical black video and silent audio therefore
still record generations `{1,2}` independently for the exact video and audio
tracks. The original editor test remains enabled; its support assertion is
strengthened to require both generations on each track, not just their union.

Missing stamps/caps, sequence replay/gaps, generation regression and queue
overflow fail explicitly. Producer publication is nonblocking. Video/audio
queues and appsrc are count-bounded at 8/64; byte/time defaults are disabled so a
normal 1280x720 frame does not falsely exceed appsrc's small default byte limit.
Partial feeder startup and Stop/drop close subscriptions and join workers.
Sticky faults are checked after worker shutdown so a concurrent fault cannot
incorrectly turn a failed recording into Complete.

### Already-live ingest lag

The concurrent target run exposed another real startup defect. Its recording
anchor was `8248392915 ns`; audio began only `14615812 ns` after it, while video
could legitimately lag by about `68447975 ns`. A negative local timestamp caused
early failure and an empty MKV, below the queue-count limits.

Before subscribing, clean recording now chooses one common origin from the
minimum of the engine anchor and each track's latest authoritative captured PTS.
Only these scalar timestamps are retained. No old buffers are replayed, and no
source/generation labels are inferred from the origin. Every pushed packet keeps
its own stamp, with `origin + local PTS == captured media PTS` for both tracks.
Start does not wait for an offline sender. A real-source regression advances
the supplied anchor by 500 ms: it failed before the fix, then produced strictly
decoded mapped segments with both tracks sharing the exact same origin.

### Independent publisher transport freeze

The private `freezePublisher` hook now freezes the publisher's real Linux
`obs-ffmpeg-mux` child, not bridge DATA traffic or the whole OBS process. It
requires one new direct child, excludes the pre-start baseline, checks exact
executable/start ticks/PPID and opens/revalidates a pidfd before SIGSTOP. Ambiguous
or absent ownership fails closed as `publisherTransportNotIdentified`.

Resume uses that exact pidfd capability, including after executable unlink or
PID replacement. Manual Stop/destruction resumes before output teardown.
Automatic resume is bounded to 30 seconds and epoch-fenced against a later
freeze. Qt/controller polling stays runnable. Standalone child-process tests
cover parent survival, ambiguity, foreign/nested/baseline children, all-thread
discovery, stale generations and destructor resume. The unlink regression
failed before the pidfd cleanup correction and now passes.

The retained verifier must still observe this hook on genuine OBS 30, receiver
quiescence with the controller online, publisher recreation/new generation, and
strictly decoded recovered A/V. An `executed` acknowledgement alone is not media
acceptance. Its receiver-data freeze remains the same network bridge mechanism
as drop and must not be relabelled an independent publisher fault.

## Executed receipts and preserved failures

All paths below are relative to the correction worktree.

| Receipt | Result |
| --- | --- |
| `var/acceptance-ingest-lag-fix-20261106.log` | Full isolated acceptance passed, including unchanged 180-second soak, strict Rust/UI checks, media/network, recording/crash recovery, browser and extensions |
| `var/target-ingest-lag-fix-20261106.log` | Imported verifier harness, actual backend/SRT drivers enabled: 65 passed, 0 failed, 0 skipped |
| `var/target-final-checkpoint-20261106.log` | Final target rerun after abort-order correction: 65 passed, 0 failed, 0 skipped |
| `var/obs-sdk-final-build-20261106.log` | Whole plugin and all component targets built with strict compiler warnings |
| `var/obs-sdk-final-ctest-20261106.log` | 11/11 non-GL CTests passed; GL/audio component executed separately from SDK-relative data path |
| `var/fixture-audio-qa.MdNgmJ/test.log` | Actual unchanged-source two-mix RMS=0 regression failure |
| `var/fixture-audio-qa.OfLlAF/test.log` | Actual corrected two-mix RMS/amplitude/leakage pass |
| `var/recovery-run-final-20261106.log` | Live HTTP daemon restart fence, old ACK rejection, budget preservation, manual Stop and unchanged outputs/Program passed |

`var/target-corrected-verifier-final-20261106.log` and failed states under
`var/recovery-http-driver.PDiOCF` / `var/target-rehearsal-driver.FiHOaC` are retained,
not superseded into passes. An initial SDK libproxy loader failure is retained
in `var/sdk-ctest-initial-dependency-failure.tar.gz`; correcting the private
loader path allowed those tests to execute, rather than skip.

The original finite-AAC-tail file remains byte-identical:
`var/recovery-local-driver.CNjaoh/strict-calibration.ts`, SHA-256
`130cc419f11937e60a41cb85e82739a142f3a3b1aa8fa926d12034d8296780cb`.
The retained verifier's finite sender drain correction, not a competing harness
patch, supplies the complete strict 150-frame calibration in the passing target
run. No scripts under its harness were changed here.

Retained Oracle review approved the final origin/abort and C++ corrections for
local checkpoint/bundling. Rust/clang LSP servers remain unavailable with prior
installation declined; no clean LSP result is claimed. Real compilers, strict
Clippy, tests and executed media supply the evidence.

## Same-verifier replay

Root should integrate this exact production checkpoint with retained verifier
`b2c6e22`, preserving its harness and original failed roots. Use fresh private
evidence directories. Build both the locked release server and plugin; CTest's
audio component uses the existing display or `xvfb-run -a` and does not skip if
that software prerequisite is absent.

```sh
source scripts/ubuntu-env.sh
cargo build --locked --release --workspace
cmake -S integrations/obs-live-production -B var/NEW-plugin-build -DCMAKE_BUILD_TYPE=RelWithDebInfo
cmake --build var/NEW-plugin-build --parallel 2
ctest --test-dir var/NEW-plugin-build --output-on-failure
cmake --install var/NEW-plugin-build --prefix "$PWD/var/NEW-plugin-stage"

target/debug/live-production-supervisor --timeout-ms 180000 --grace-ms 10000 -- \
  cargo test --locked -p live-production --test editor_clean_provenance --test editor_clean_metadata_free
target/debug/live-production-supervisor --timeout-ms 180000 --grace-ms 10000 -- \
  cargo test --locked -p live-production-media --test runtime_recording_contract \
  --test source_capture_recording_contract --test source_capture_lag_contract
target/debug/live-production-supervisor --timeout-ms 300000 --grace-ms 10000 -- \
  cargo test --locked -p live-production-media --test srt_drift_endurance
```

Then rerun the verifier-owned genuine OBS rehearsal/full recovery entrypoints
against `NEW-plugin-stage`, with all original absolute RMS/tone, provenance,
fragment-count, strict EOS/decode and safety assertions. The verifier owns the
harness change needed to invoke the now-real freeze hook; it must not merely
remove an unavailable flag or omit a required case.

The primary Studio executable/bundle/state/recordings and original checkpoint
refs remain preserved. PID/start ticks remain app `419353/36675901`, supervisor
`419348/36675900`. Only actual laptop/network, physical capture/hardware, intended
editor application and explicitly authorized provider rehearsal remain external
inputs; they are not excuses for the native software replay above.
