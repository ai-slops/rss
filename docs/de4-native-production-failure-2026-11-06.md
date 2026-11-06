# Root relay: actual Ubuntu de4 replay

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
