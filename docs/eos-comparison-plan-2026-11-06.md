# Predeclared retained-native EOS comparison

This plan is declared before executing the cases. Overall acceptance stays
partial. Production remains byte-identical to de4; primary owns fixes.
The original 196.93-second failure has no retained raw media. Nothing below
reconstructs or claims knowledge of that missing file.

## Fixed four-case matrix

Run each exactly once, in this order, on the same retained VM:

1. `idle-minimal`: no extra CPU load, no `GST_DEBUG` override.
2. `idle-traced`: no extra CPU load, GST event/bus/queue/mux/state tracing.
3. `loaded-minimal`: one owned busy-loop Node worker, no `GST_DEBUG` override.
4. `loaded-traced`: one owned busy-loop Node worker, the same tracing as case 2.

All four use the same compiled original `srt_drift_endurance` test, timestamp
injection, 168-second initial phase, disconnect/reconnect, original >=180-second
duration and rate/RSS/frame/queue/fragment checks. Production's strict 15-second
EOS wait, Null transition and strict decode remain unchanged. The external
supervisor bounds each test to 300 seconds, not a larger EOS deadline. Load
workers have independent ownership and a finite deadline; they stop before
post-run media inspection. Cases do not stop when green and are not retried.

Private `TMPDIR` is nested in each case so a forced process death leaves raw
temporary artifacts inside the archive. Existing unwind-safe retention captures
normal failures. Distinct case evidence paths prohibit overwriting earlier runs.

## Hypotheses and discriminating observations

- Rejected/lost upstream EOS: explicit rejection, missing branch EOS events, or
  branch-specific last progress followed by the unchanged timeout.
- Accepted EOS blocked on one branch: native traces show EOS on one branch but
  no corresponding mux/queue EOS on the other before the Stop return.
- Both branches drain but asynchronous mux finalization misses the deadline:
  both branch EOS events, fragment-close sequence and pipeline bus messages
  distinguish it from upstream loss.
- A bus Error rather than timeout: native bus error details and timing identify
  this path. Current production `timed_pop_filtered(Eos, Error)` uses the same
  generic finalization error for a non-EOS message and for no message.
- Scheduling sensitivity: the fixed logging/load contrast may alter occurrence;
  a pass under any condition never clears a prior failure or proves a cause.

Test-only observations record wall/monotonic Stop entry/return, public output
state/progress/error before/during/after Stop and the final stopped-state wait.
No diagnostic thread reads or consumes the pipeline bus; native GST debug
categories observe events without competing with the production bus owner.
No pipeline pointer, unsafe injection or production edit is introduced.

The retained passing trace from the earlier run is a comparison only: its final
reference EOS occurs near 180.731 s, both mux queue EOS events follow, fragment
36 closes near 180.733 s and all 37 segments decode. The failed first run has
no such event timeline. Any new failure gets its own event timeline, byte hashes,
FFprobe counts and strict raw decode outcomes. Raw decode alone never promotes
a Failed session to Complete.

## Artifact journal

- Test-only `drift_support/eos.rs`: optional output Stop boundary observer.
- Existing fixture `finish`: observer opt-in; original stop/wait/check path kept.
- `scripts/target/eos-soak-matrix.mjs`: fixed cases and exact process ownership.
- `scripts/target/harness/eos-media-evidence.mjs`: raw file/decode evidence.
- Bounded guest/host runners: same overlay, new private evidence root, no updates
  to Studio, accounts, operator network or credentials.
- Ignored `var/de4-eos-matrix/`: full logs/media/state, including potentially
  sensitive native traces. Retain, do not publish. Environment overrides are
  child-scoped; owned workers and VM are stopped at run completion.

No next production checkpoint is assumed. The already-passed audio/provenance
and safety gates remain recorded regressions, not silently rerun or relabelled.
