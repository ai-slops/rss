# Same-verifier bounded progress: recovery review and EOS ownership

## Task remains open

**This is a progress checkpoint, not completed native acceptance.** The earlier
failed genuine OBS audio, reconnect provenance, bounded Program recording,
180-second recorded-media/RSS gate and publisher-only freeze remain failed or
unavailable. Physical targets are separate external gates, not explanations for
these software failures. All prior raw media and original assertions remain.

No main merge/push, publication, deployment, force operation or public/provider
broadcast occurred. Root resumes/owns production corrections and publication.

## Verified exact import and local integration

- Root-delivered code-only bundle:
  `/home/dim/tasks/verification-results-20261106/primary-recovery-review.bundle`.
- Bundle SHA-256 verified:
  `2e8cbaaef71c456a91c7a08a5f35bc861ae8ee28e8a6fb65fa49bb126542ec87`.
- Required base verified: `9a07ef4beda93505a4c0d50485bc3c5f9df5df62`.
- Imported isolated ref `import/primary-recovery-88e0cd7` at exact
  **`88e0cd7ceefe3e3ce2608a9510c189612bab2402`**.
- New local worktree `/workspace/ubuntu-recovery-review`, ordinary branch
  `work/ubuntu-recovery-review-20261106`. Merge
  `8a8ed89f3b06abb11658ab4b69bc48fd153de048` combines that exact code with
  retained verifier `b2c6e22f3fc42206803cf1c5a72bb75321131489`.
- Original `b2c6e22`, its tested `1228a8d`, all older branches/indexes, failed
  trials and media are preserved. No second workspace or VM was provisioned.
- Final `server/src`, `media/src` and `integrations/` blobs match exact `88e0cd7`;
  production code was not edited by the verifier.

Execution source map:

| Exact verifier code | Executed work |
|---|---|
| `e303438f4a6809086a0148d3490516c7bc8ef765` | Native builds, 13 recovery Rust tests, HTTP test, first live run-fence probe, six-case AAC matrix, real OBS controls, Clippy |
| `72fbe5a02548842678760ee46544a5b86a8f68b4` | Stronger sender/receiver calibration and detailed recorder trace; new probe's invented no-op Take requirement failed and was retained |
| `037d6d521f0585ad7d0b239349306711dbdb98cd` | Corrected live run-fence probe passed, plus content-unit regression |
| `42a8fddcfae05e4f4937114152ab2505658e1ed4` | Added pure completeness classifier, tested against all six retained matrix outcomes |

The full hash for each guest execution is retained in its `source-head.txt` or
the Git bundle. The no-op Take correction removes a verifier-invented requirement,
not a production assertion or media check.

## Same retained Ubuntu execution

The same qcow2 overlay, backing image and pinned SSH identity were resumed for
owned bounded runs, then stopped. KVM/q35/host CPU, 4 vCPU/8192 MiB; native Ubuntu
24.04.5, kernel 6.8.0-147, Rust/Cargo 1.91.1, GStreamer 1.24.2, FFmpeg
6.1.1-3ubuntu5 and existing native rswebrtc 0.14.3. No host network/account,
operator credential, existing Studio or other workspace changed.

The existing reviewed foreground runner supplied exact-child cleanup. Builds
used separate guest checkout `/home/validator/review88`; old failed checkout and
media remain intact. No new dependency installation was needed in this slice.

## Persisted recovery binding: executed native results

- Native locked debug/release build: **exit 0**.
- Recovery Rust filter: **13 passed**, including old-run ACK/health cancellation,
  legacy run binding, preserved retry budget and failed batch rollback.
- Existing recovery HTTP integration: **exit 0**.
- Native formatting and strict all-target Clippy: **exit 0**.
- Actual live TCP/backend/GStreamer run-fence probe: **pass**, explicitly not OBS
  media acceptance.

The live probe establishes real decoded media in engine A, freezes its owned
synthetic sender while controller polling remains online, obtains a pending
attempt-1 command, then restarts only its daemon against the same SQLite state.
On engine B it observes startup cancellation, preserved attempt count 1 and
rejection of the exact old duplicate. New engine B media then reaches continuity
generations 1 and 2. A late old `started` ACK is ignored: attempts remain 1, and
old health does not turn it into `recovered` or reset the budget.

The corrected final receipt reports actual engine-B generation-2 progress:
18 video frames, 30 audio buffers and positive advancing PTS. Four strict clean
segments contain H.264 1280x720 plus AAC stereo 48 kHz, changing red fixture
content and dominant 440 Hz; RMS approximately 0.796. Program content and output
count remain unchanged. This is **real-backend/GStreamer comparison evidence**,
not a substitute for the still-failed genuine OBS audio path.

Private proof:
`var/recovery-review-03/engine-run-live/report.json`.

## Program content fence correction

The new helper compares persisted project/canvas/assets, actual applied Scene,
scene identity/revision and live audio-route settings. Meter observations and
timestamps are not content. Within one engine run it still requires the exact
Program revision token to remain unchanged, catching an unintended identical-
pixel Take. Across engine runs it permits a reissued token **only if the real
content fence remains identical**.

Four regression tests pass. They first exposed three failures in the old raw-
revision-only rule: restart/token handling and missed real content changes.

The first version of the additional live probe incorrectly required taking the
already-applied Scene to advance revision. That command is legitimately a no-op
here. Its failed run is preserved; the corrected probe records
`sameSceneTakeAdvancedRevision:false`. Raw revisions also happened to remain
equal in this native sample, so token reissue is covered by the regression,
not falsely claimed as an observed native transition. Scene/content comparison
was still executed across actual different engine-run IDs.

Real OBS control diagnostics with `88e0cd7` pass reconnect, offline/online lease,
maintenance/source-disable suppression and server-restart budget/action fences.
They remain **partial, exit 1, obsClaim false**, because their initial strict
media check still rejects silent audio (RMS about `3.61e-6`).

## Finite AAC: sender/receiver drain ownership

Six cases were chosen in advance, not rerun quietly until green: idle/owned CPU
load crossed with no sender drain, one-second receiver-only waiting, or a 500 ms
sender drain. Each retains the exact post-mux source TS and received TS, FFprobe,
strict decoder outcome and source/receiver supervisor exits. Authentication is
private, loopback-only and generated per case. Both load workers are owned and
bounded. No OBS acceptance claim is made by this matrix.

| Case, repeated idle and loaded | Source frames V/A | Received frames V/A | Strict decoder | Interpretation |
|---|---|---|---|---|
| Sender drain 0, receiver delay 0 | 150 / 235 | 147 / 229 | 0 | Complete access units can still be lost |
| Sender drain 0, receiver delay 1000 ms | 150 / 235 | 147 / 229 | 0 | Waiting after sender close cannot restore them |
| Sender drain 500 ms, receiver delay 0 | 150 / 235 | 150 / 235 | 0 | Full measured fixture transfer retained |

All source copies are complete and strictly decodable. Sender and receiver
processes exit zero in all six cases, so process success and strict decode alone
are insufficient for finite delivery completeness. Receiver bytes are not
required to hash-match source bytes because the SRT path adds transport stream
headers; the measured complete paths have 376 extra TS bytes but identical
decoded frame counts.

This isolates the observed loss to delivery/teardown after a complete mux output,
not a malformed encoder source. It supports keeping the sender/socket alive
through the negotiated live-latency tail. It does not claim a universal 500 ms
network bound or a byte-for-byte diagnosis of primary's separate truncated AAC
artifact: that private `CNjaoh` media was not in the code-only bundle. Its reported
SHA and failure remain in primary's receipt, not relabelled by these runs.

The actual local recovery calibration now retains `strict-source.ts` and compares
both received video **and audio** frame counts to that sender capture while
preserving full strict decoding and exact 150-video-frame requirement. Its real
native driver passes. A pure completeness regression rejects missing AAC,
unknown counts and whole AAC/video loss even when `strictDecoded:true`.

All source/receiver files and six matrix rows:
`var/recovery-review/aac-matrix/`. Strengthened calibration driver:
`var/recovery-review-02/finite-calibration.log` and its retained
`var/recovery-local-driver.*` guest directory.

## Program recorder: concrete production failure for prompt relay

The unchanged runtime recording test was rerun once with bounded diagnostic
logging and media preservation. It still fails, now at the original nonempty-
fragment assertion under this scheduling profile. At approximately **0.355 s**:

```text
matroskamux: Caps changes are not supported by Matroska
Current: H.264 960x540, colorimetry=bt601, chroma-site=jpeg
New:     H.264 960x540, colorimetry=2:4:5:1, different codec_data
splitmuxsink: New caps were not accepted. Switching output file
```

Dimensions did not change in this observed rejection. The color metadata and
SPS/codec-data changed mid-stream. An empty recording fragment remains, and the
strict original assertion rejects it. No error was ignored and no empty tail was
trimmed. This gives primary an actual native negotiation failure to investigate
at the recording/encoder/mux boundary rather than an unsupported guess about
physical hardware, Program pixels, or final-only EOS loss.

Full private log and raw copied media:
`var/recovery-review-02/program-recording-trace.log` and
`var/recovery-review-02/media/runtime-recording/`.

## Genuine publisher-only freeze

`docs/publisher-freeze-interface-request-2026-11-06.md` specifies the exact
minimal interface requested from primary, including per-instance packet hold,
bounded lifetime/generation cancellation, actual-delivery counters and required
native proof. Existing Publisher handles are private and only lifecycle
start/stop/restart primitives are exposed; none safely performs this fault.
The verifier did not introduce an unsafe global pause, memory patch, source mute
or bridge drop and call it independent publisher freeze.

The hook remains unavailable and cannot be promoted to pass. No production
implementation was simultaneously edited.

## Evidence, stopped runtime and next checkpoint

Private artifacts are extracted under this worktree's `var/recovery-review*`
and retained inside the stopped guest. Archives include private fixture state;
never publish them. Code-only local bundles contain no state/database/token,
SSH key, disk, raw receipt or media. Exact SHA/result/timings are in each private
TSV and source-head record.

| Private archive | SHA-256 |
|---|---|
| `recovery-review-evidence-20261106.tar.gz` | `c8442dfad82e972db4b3a0d7ed0dd86a8951cdbf7ea3c6d9bf7569733070e48a` |
| `recovery-review-evidence-02-20261106.tar.gz` | `24270a36dcf434518277244a4a31fc4f50d47c4ea67b90fcc1e387d93755abf1` |
| `recovery-review-evidence-03-20261106.tar.gz` | `9a861777f09e4be00dda722e0674c3f8e075a4a3355a08936628fb680e722836` |

The same retained VM's final QEMU PID **20293** stopped at
**2026-11-06T02:55:56Z**. Runner zero is the final corrected individual probe's
outcome, **not task completion or overall acceptance**. No automatic boot/retry
is scheduled. The overlay, SSH identity, every failed media file and earlier
evidence remain retained.

Continue here after root relays exact production corrections. Remaining genuine
OBS silence, reconnect provenance, Program/soak recorder failures and the true
publisher-freeze seam are not cleared. Target laptop/editor/device/provider
acceptance remains separately unobserved; it does not excuse software failures.
LSP access remains restricted to the original request cwd; compiler, Clippy and
actual native execution are the available evidence, not an invented LSP pass.
