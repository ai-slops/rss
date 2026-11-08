# In-process publisher transport and ordered recorder EOS

## Scope and retained evidence

This correction is based on production
`de4b80058871f4ec5f138ddb25cb68d05c5bc718`. The genuine OBS30 audio, per-track
provenance, black/silent reconnect, Program, causal and lag behavior established
by the retained verifier is preserved. No solved audio/provenance implementation
was changed here.

The code-only replay bundle was verified with SHA-256
`1c2b73a8b757d61624751e3da5192af32c0fdeddc2ea78cd953835a6312f047c` and imported
as `review/de4-native-replay-20261106` at
`078c2e2ed24b235ca6e87243382118d325b62e84`. Both safe relay documents were read.
Its executed integration is `7709ded2d4e4d7e9a5969f444eee104e15f14040`.
No private archive, credential, verifier session or VM was accessed.

The original 180-second native soak failed after 196.93 seconds because the
recorder did not finalize with EOS. Its temporary media was lost during unwind;
that evidence gap remains explicit. The later instrumented passing run with no
production change does not clear the failure. The same verifier owns its new
unwind-safe retention and diagnostics. No competing harness was edited here.

## Compatible transport ownership

Ubuntu OBS30 defaults `ENABLE_NEW_MPEGTS_OUTPUT=ON`: its stock MPEG-TS output
writes through in-process AVIO/libsrt, not `obs-ffmpeg-mux`. The old pidfd seam
therefore could not perform this fault. It is no longer linked into the product;
its standalone historical process-capability test remains, not a fallback.

The module now registers `live_production_mpegts_srt`, a public-libobs encoded
output with one owned writer/session per Publisher. It retains the existing
independent view, OBS x264 encoder, explicit AAC mix and 48-kHz stereo profile.
The module owns FFmpeg MPEG-TS, custom AVIO and direct libsrt. There is no system
OBS patch, private OBS ABI, global I/O interposer, bridge substitution, source
mute, arbitrary thread/process suspension, or forced legacy backend.

Host, port, passphrase, stream ID, latency and encoding settings are private
in-memory output settings. Credentials are applied directly through libsrt
options, not embedded in a logged FFmpeg URL. Source recovery remains the sole
Publisher recreation authority; OBS automatic reconnect is disabled.

Encoded callbacks perform bounded payload copies and queue admission, never
network I/O or capture termination. The queue is limited to 64 packets/4 MiB.
Overflow and encoder failure wake the worker, which signals one error outside
the callback. Stop interrupts DNS/connect/send, wakes the queue, joins the sole
worker and ends capture before releasing its resources. Each transport balances
its libsrt runtime reference after closing its socket.

Literal addresses bypass DNS. Qt hostname resolution and all deduplicated
address attempts share one three-second deadline. At most eight candidates are
attempted, with remaining time fairly divided so a dead first AAAA address
cannot starve IPv4. Cancellation wakes the resolver promptly. The deterministic
fallback regression failed before fair sharing and now passes.

## Per-publisher freeze contract

The private fixture's existing `freezePublisher` command invokes procedures on
that exact output object:

```text
live_production_freeze(out bool success, out int epoch)
live_production_resume(int epoch, out bool success)
```

Freeze closes admission, drops queued encoded work and arms a new epoch.
Already-processing mux work is allowed to finish before acknowledgement;
FFmpeg interleave, MPEG-TS payload and AVIO buffers are flushed within that
processing scope. Freeze confirms both formatting and actual SRT sends are
quiescent within 500 ms. A timeout is failure, not acknowledgement of a future
freeze. Failed freeze invalidates old work and retains the IDR gate.

Packets are epoch-stamped. A popped old packet cannot begin processing or send
after Freeze/Resume. Resume is IDR-gated and stale/prior-session epochs are
rejected. Repeated matching operations are idempotent. PublisherRuntime adds an
outer monotonic lease so a delayed fixture timer cannot resume a replacement
Publisher. Resume is bounded to at most 30 seconds by the finite fixture; Stop
quiesces and destroys only its own session.

Qt/controller processing is not suspended. Another publisher continues sending.
The report acknowledges `executed` only after writer quiescence. Failure is
`publisherTransportNotQuiescent`, not an invented child-process prerequisite.
The acknowledgement itself is not recovered-media evidence: the retained
verifier must still observe receiver stall, online controller, recreation/new
generation and strict usable A/V.

## OBS30 and FFmpeg compatibility

SRT connects before capture begins. Encoder headers are retrieved only after
the first actual encoded packet, matching the OBS30 lifecycle. Same-output
Stop/Start uses a fresh session with monotonic public freeze epochs.

OBS packet timestamps use denominator ticks: for 30000/1001, consecutive PTS
values advance by 1001. Both PTS and DTS therefore divide by `timebase_num`
before rational rescaling, matching OBS30's stock MPEG-TS implementation.
The regression distinguishes the incorrect 3006003 ticks from the correct
3003 ticks at 90 kHz, preserves negative DTS/NOPTS and unit-rate audio, and runs
real default and NTSC encoders. AVIO callback signatures are version-guarded for
FFmpeg 6 versus newer releases. Only public OBS output/encoder/proc APIs are used.

## Recorder finalizer ownership

Both Program and source recorders now use `async-finalize=false` with an explicit
`muxer=matroskamux`. Supplying only `muxer-factory` would be ignored in this mode
and could silently select MP4; runtime tests now verify the actual muxer and
on-disk EBML bytes as well as strict decoding.

A controlled GStreamer barrier holds a previous asynchronous fragment sink's
EOS while a new sink exists. `fragment-closed` can be advertised while pipeline
EOS still waits for that detached finalizer. The synchronous case keeps one
in-pipeline sink and completes in pipeline order without that external owner.
Separate probes require exactly one EOS on both source tracks and both live
Program inputs. This is mechanism evidence, not a claim to reconstruct the lost
historical native interleaving.

The original 15-second EOS deadline, bounded fragments, keyframe requests,
Matroska output, raw recovery and strict decoder assertions remain. No timeout
increase, retry wrapper, fragment trimming, assertion relaxation or pass
reclassification was introduced.

## Executed local evidence

Artifacts are relative to `/workspace/main/var/remaining-verification-20261105`.

| Receipt | Result |
| --- | --- |
| `var/transport-eos-final-acceptance-20261106.log` | Full isolated acceptance passed, including the original 180-second soak, strict Rust/UI checks, recording, media/network, browser and extensions |
| `var/transport-eos-reviewed-build-20261106.log` | Whole plugin and all native component targets built with strict warnings |
| `var/transport-eos-reviewed-ctest-20261106.log` | 14/14 non-GL CTests passed |
| `var/transport-eos-reviewed-component-30-20261106.log` | Actual Publisher and direct output, two authenticated SRT receivers, independent freeze, restart, bad auth and strict per-session H.264/AAC/tone checks passed |
| `var/transport-eos-reviewed-component-ntsc-20261106.log` | Same component passed at 30000/1001 |

Local runtime versions are explicitly OBS 31.1.2, FFmpeg 8.0.1, libsrt 1.5.3 and
GStreamer 1.26.9. They are not represented as Ubuntu OBS30/FFmpeg6 execution.
The retained Oracle approved the final implementation for checkpoint/bundling.
Unavailable Rust/clang LSP servers were previously declined; compiler and strict
Clippy evidence is retained rather than inventing clean LSP output.

Component captures are kept per SRT connection, not concatenated across restart.
Demux corruption/non-EOF errors are rejected, codecs use `AV_EF_EXPLODE` and EOF
is drained. Each session requires real video/audio, absolute RMS/tone amplitude
and coherent durations. A failed strict capture is retained privately rather
than deleted. Earlier local logs from the test-construction and Clippy failures
remain under `var/`; none is silently promoted into a pass.

## Same-verifier entrypoints

Root owns review/integration/publication and relays this exact checkpoint to the
same retained verifier. Use native Ubuntu development packages, never the Alpine
sysroot. In addition to existing OBS/Qt dependencies the new backend requires
`pkg-config`, `libavformat-dev`, `libavcodec-dev`, `libavutil-dev` and
`libsrt-openssl-dev` (or the matching native libsrt development variant).

```sh
source scripts/ubuntu-env.sh
pkg-config --modversion libavformat libavcodec libavutil srt
cargo build --locked --workspace
cargo build --locked --release --workspace
cmake -S integrations/obs-live-production -B var/NEW-transport-build -DCMAKE_BUILD_TYPE=RelWithDebInfo
cmake --build var/NEW-transport-build --parallel 2
QT_QPA_PLATFORM=offscreen ctest --test-dir var/NEW-transport-build --output-on-failure
cmake --install var/NEW-transport-build --prefix "$PWD/var/NEW-transport-stage"

GST_DEBUG=splitmuxsink:5 target/debug/live-production-supervisor \
  --timeout-ms 300000 --grace-ms 10000 -- cargo test --locked \
  -p live-production-media --test srt_drift_endurance -- --nocapture

scripts/target/recovery-native.sh --plugin-dir "$PWD/var/NEW-transport-stage" \
  --evidence-dir NEW --server-binary "$PWD/target/release/live-production" \
  --supervisor-binary "$PWD/target/release/live-production-supervisor" \
  --web-dist "$PWD/web/dist" --obs-binary /usr/bin/obs \
  --xvfb-binary /usr/bin/Xvfb --deadline-seconds 300
```

Leave local SDK overrides unset on Ubuntu; encoder modules are discovered
through OBS's public module paths. The default and NTSC components use an
existing display or owned `xvfb-run -a`. Retain the verifier's unwind-safe soak
artifacts and bounded diagnostics on either outcome. Require the full recovery
flow with no omissions and actual frozen receiver/recreated usable A/V; do not
substitute an acknowledgement or a child-process inventory for this proof.

No main/protected merge, push, force, deployment or public/provider broadcast
was performed. Studio and prior checkpoints remain preserved. Physical laptop,
editor and provider targets remain separate and do not explain these software
gates. The original native EOS failure and unsupported de4 freeze are not closed
until the retained verifier executes this corrected checkpoint.
