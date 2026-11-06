# Root relay: actual Ubuntu de4 replay

Final measured disposition and complete command/evidence map:
`docs/de4-native-replay-2026-11-06.md`. The chronology below is retained; its
earlier “continuing” statements are not the final state. The full flow ultimately
reaches partial solely for unavailable publisher-only freeze. EOS failure remains
unresolved despite a later passing soak.

Production checkpoint: `de4b80058871f4ec5f138ddb25cb68d05c5bc718`.
First executed clean integration: `81e674c` (full SHA retained in private
`var/de4-native/source-head.txt`). Its `server/src`, `media/src` and
`integrations/` trees exactly match de4. Same retained KVM overlay; Ubuntu
24.04.5, OBS 30.0.2.1, GStreamer 1.24.2, FFmpeg 6.1.1.

## Concrete production failure requiring primary

At 2026-11-06T10:47:26Z the original 180-second drift/RSS contract was started:

```sh
source scripts/ubuntu-env.sh
target/debug/live-production-supervisor --timeout-ms 300000 --grace-ms 10000 -- \
  cargo test --locked -p live-production-media --test srt_drift_endurance -- --nocapture
```

It failed after 196.93 seconds, exit 101:

```text
Error: output pipeline failed: recording pipeline did not finalize with EOS
test sustained_independent_clocks_report_drift_and_preserve_bounded_progress ... FAILED
```

No strict EOS/decode assertion or timeout was weakened. This candidate does not
pass the soak, despite the earlier I420/88e fixture pass. The first failed run's
temporary recorder directory was removed during unwind before the old
post-success preservation call could run. That is an evidence limitation, not
retained raw-media proof. The verifier is adding an unwind-safe test-only
retention guard and bounded mux diagnostics for the next replay. No production
fix is being made concurrently.

## Genuine new-candidate successes, not OBS31 substitution

### Subsequent live freeze-hook blocker, 11:15 UTC

At clean integration `d72e68f`, the hook itself was invoked, not just inferred
from a process scan. Its exact result was `publisherTransportNotIdentified`.
The verified OBS parent executable was `obs` and both its direct-child and mux
candidate lists were empty. Controller polling remained running. Private proof:
`var/de4-native-04/recovery-full/private/publisher-freeze-progress.json`.

Official OBS30.0.2 commit `afbebf94867982b7c294ebc1b8c04438f8b59536`
defaults `ENABLE_NEW_MPEGTS_OUTPUT=ON` in
`plugins/obs-ffmpeg/CMakeLists.txt`. Its `obs-ffmpeg-mpegts.c` implements
`ffmpeg_mpegts_muxer` using in-process libsrt/AVIO and pthread writes. The
`obs-ffmpeg-mux` child belongs to the separate legacy `obs-ffmpeg-mux.c` path.
Primary's new hook therefore cannot perform this required fault on the actual
Ubuntu OBS30 configuration. No whole-process/thread stop, source mute or bridge
DATA pause was substituted or labelled independent publisher freeze. This needs
a primary-owned correction compatible with the actual publisher output.

The same full run recovered real strict OBS A/V after bridge DATA drop (generation
1 to 2), duplicate delivery and the separate DATA-freeze case (generation 2 to
3). Its later whole-process-resume path failed because the verifier bridge had
not authorized a new peer at that fault boundary (36 rejected peers). That is
an owned harness issue being corrected, not evidence that production recovery
itself is broken.

The diagnostic safety modes all pass their individual strict-media, auth,
process/poll/intent, duplicate, backoff, concurrent retry, Stop, lease and daemon
restart assertions. Their report status remains deliberately partial/exit 1.
The instrumented soak replay passes all original assertions at 181 seconds,
5409 frames, 37 segments, RSS growth 223676 KiB and PCM RMS 2789.24. This does
not erase or clear the initial EOS failure; no production change occurred between
the failure and pass.

- Native debug/release server and strict plugin builds passed.
- All 12 native CTests passed, including real libobs audio and pidfd fault tests.
- Genuine OBS30 two-source rehearsal passed original absolute audio/tone,
  changing-video, complete-fragment, strict decode and negative publisher checks.
- Original clean reconnect provenance and black/silent metadata-free tests
  passed, including generations `{1,2}` on **each** audio/video track.
- Bounded Program, causal source capture and already-live lag contracts passed.
- Output lifecycle tests, recovery unit/HTTP tests, target tests, formatting and
  strict all-target Clippy passed.
- Live engine-run restart probe passed old ACK/duplicate rejection, budget
  preservation, recovered generations and unchanged actual Program content.

## Independent checks continuing

Recovery entrypoints were blocked before native execution because the isolated
verifier's web dependency cache link did not supply `tsc` or `web/dist`. This is
an owned runner setup defect, not a production recovery failure. The verifier is
installing locked dependencies in its new private checkout and continuing the
real publisher freeze, controller-alive/quiescence/recreation/media proof and
the auth/epoch/duplicate/backoff/concurrency/manual-Stop cases.

Private archive `de4-native-evidence-02.tar.gz` SHA-256:
`00a55e71fca41e4080fd81d12f6ef22dcde205ac06b9abb2bbb80b96af1b02db`.
It contains credentials and must remain private. The same VM was stopped at the
end of that owned run. No Studio, account, operator network or public/provider
output was changed. Overall native acceptance remains open.
