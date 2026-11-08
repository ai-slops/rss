# Pending backend-compatible native freeze proof

Primary owns the actual transport seam and its contract. No speculative command
or field is being added to the production plugin. The current native result
`publisherTransportNotIdentified` and `requiredUnavailable` stay unchanged until
a focused checkpoint is relayed and executed. Child-process-only observations
must not be reused as proof for an in-process backend.

## Adapter boundary after the exact contract is delivered

Keep the existing native recovery setup, scoped source/controller credentials,
unchanged source fixture and receiver API. Replace only the transport-observation
portion of `recovery-publisher-freeze.mjs` according to primary's real contract.
The adapter must identify the actual publisher instance/run and fault epoch and
retain the actual backend delivery/hold/resume observations it supplies. Do not
invent these fields from an acknowledgement, receiver color, or current source
state. Reject stale/ambiguous/unsupported ownership. A receipt saying `executed`
is necessary control evidence at most, never the media outcome.

## Required native cases and retained assertions

1. **Independent fault and recreation:** establish advancing video/audio/PTS and
   strict fixture A/V first. Pause only action dispatch, then invoke the finite
   publisher transport fault. Keep bridge DATA enabled with no injected drops;
   keep the fixture generating unchanged tones/video. Observe controller online
   on every sampled status and advancing plugin poll sequence throughout the
   hold. Require receiver counters/PTS to become stable after bounded drain.
   Preserve nonempty pending recovery ID. Release dispatch through an explicit
   one-shot peer handover only for genuine publisher recreation. Require exactly
   one execution, fresh publisher lifecycle identity, new receiver generation,
   two healthy same-generation video/audio/PTS advances and unchanged Program.
   Capture >=3 seconds strictly decoded recovered H.264/AAC with motion, color,
   absolute RMS/tone checks. Recheck stable new publisher identity/execution count,
   controller liveness and live receiver progress after capture.
2. **Finite automatic resume:** with recovery dispatch safely held or policy
   explicitly maintained, observe real quiescence, then actual delivery and
   usable A/V resume within the seam's declared bound. Restore policy explicitly.
   Do not infer a resume from timer acknowledgement or from expiry alone.
3. **Epoch fencing:** hold epoch A, then stop/recreate into a fresh publisher/run
   and begin epoch B. Deliver A's late resume using the supported contract. B must
   remain held/quiescent while its controller stays online. Only B's matching
   resume may restore delivery. An old timeout must not thaw the new publisher.
4. **Idempotence:** duplicate the same fault and resume command/epoch via the
   primary-owned contract. Require the same outcome with no second execution,
   extended hold, generation jump or new publisher side effect. Unknown/stale
   epoch requests must fail closed without changing media behavior.
5. **Manual Stop cleanup:** while held and with a real queued recovery command,
   Stop must cancel it, disarm local desired state and release fault resources
   without accidentally restarting output. Repeat Stop; resume dispatch and wait
   beyond the old hold's bound. Receiver counters/PTS remain quiescent and restart
   count remains unchanged. A subsequent deliberate fresh start is a new intent
   and must produce strict usable A/V without inheriting the old hold.

If primary exposes a distinct local per-publisher hold entrypoint, test only the
owned fixture instance and keep any sibling publisher/control/backend progress
observable where the delivered fixture supports it. Never implement an arbitrary
thread stop, global OBS stop, source mute, receiver drop or bridge pause as the
transport fault. Existing whole-OBS SIGSTOP and receiver DATA cases retain their
different labels and cannot satisfy this gate.

## Integration and regressions

Inspect and archive any dirty verifier work before a new isolated integration.
Verify the relayed bundle/hash/prerequisite and exact production trees. Preserve
all failed media, earlier checkpoints and proof receipts. On the same native
OBS30/GStreamer1.24 overlay, build both server and plugin and rerun genuine audio,
both per-track `{1,2}` provenance tests, bounded recording, original full soak,
freeze cases above, full auth/epoch/duplicate/backoff/concurrency/restart/Stop
flow, formatting and strict checks with no omissions. Compare real Program
content across explicit engine restart and enforce raw revision equality inside
each run. The optional export-package fixture skip stays not executed until its
actual fixture is supplied. No acceptance pass exists while EOS or the native
freeze proof remains unresolved.
