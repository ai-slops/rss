# Source-scoped publisher recovery contract, version 1

Source-scoped recovery uses the versioned protocol below.

## Boundary and ownership

The existing server coordinator owns per-source recovery policy, authenticated
enrollment, actual receiver observations, retry budget, and command fencing.
The existing OBS module owns its independent Publisher and local operator intent.
OBS makes outbound HTTPS polls with a source-only controller credential. There is
no inbound OBS server or companion daemon. A restart recreates only the independent
Publisher/view/encoders for the already-selected source and stereo mix.

OBS/plugin process restart is **unavailable** without a separately supervised
endpoint, which is not enrolled here. After OBS/plugin relaunch, publishing stays
stopped until a fresh manual Start, preserving the existing no-autostart behavior.
Persisted policy or old running intent alone must never start a new process.

## Operator-cookie API

Existing operator authorization plus `x-live-production: 1` on mutations:

- `GET /api/v1/sources/{id}/recovery`: nonsecret status below.
- `PUT /api/v1/sources/{id}/recovery`: body
  `{expectedPolicyVersion:"1",enabled:true,maintenance:false}`; returns status.
  First policy version is `"1"`; stale version is HTTP 409. Defaults disabled.
- `POST /api/v1/sources/{id}/recovery/enroll`: `{rotate:false}`; returns
  `{schemaVersion:1,sourceId,controllerId,controllerToken,controlPath,policyVersion}`.
  `controlPath` is `/api/v1/source-recovery/{id}/poll`, never an inferred origin.
  Return token once with `Cache-Control: no-store, private`. Existing enrollment
  requires explicit `rotate:true` or HTTP 409; rotation/revoke invalidate old tokens.
- `DELETE /api/v1/sources/{id}/recovery/controller`: revoke, return status.
- `POST /api/v1/sources/{id}/recovery/retry`: `{requestId:"uuid"}`, return status.
  Deduplicate request IDs. Reject with HTTP 409 when disabled, maintenance, source
  disabled, controller offline/unarmed, or local desired stopped. It never sets
  running intent. A new explicit eligible retry can reset an exhausted incident.

Only existing SRT sources support enrollment. Source deletion/disable makes
controller actions unavailable. Store typed state under `plugin_storage` namespace
`source-recovery.v1`, keyed by source ID; do not add fields to every Source model.
Persist only the token SHA-256, never return it in snapshots/status/receipts.

Status response shape (all listed fields present; unknown counters/times nullable):

```json
{
  "schemaVersion":1,"sourceId":"id","policyVersion":"1","sourceEnabled":true,
  "policy":{"enabled":false,"maintenance":false},
  "controller":{"enrolled":false,"controllerId":null,"online":false,
    "processEpoch":null,"localDesired":"stopped"},
  "receiver":{"state":"unknown","continuityGeneration":null,
    "videoFrames":null,"audioBuffers":null,"lastVideoPtsNs":null},
  "retry":{"attempts":0,"maximumAttempts":3,"nextEligibleAtNs":null,"lastResult":"never"},
  "capabilities":{"publisherRestart":true,"processRestart":false,
    "freezeDetection":"decoded_counter_and_pts_progress"}
}
```

Receiver states: `unknown`, `warming`, `healthy`, `stalled`, `source_disabled`,
`controller_offline`. Retry results: `never`, `pending`, `started`, `failed`,
`recovered`, `exhausted`, `cancelled`. No secret fields in these responses.

## Source-only controller API

`POST /api/v1/source-recovery/{id}/poll`,
`Authorization: Bearer <source-specific-controller-token>`; no operator cookie.
Authenticate before revealing source state. Token cannot call operator APIs,
other sources, publishing credential retrieval, public outputs, or generic commands.
Request JSON (decimal-string nonnegative integers; IDs UUIDs; bounded body):

```json
{"schemaVersion":1,"controllerId":"uuid","processEpoch":"1","pollSeq":"1",
 "intentEpoch":"1","localDesired":"stopped","publisherState":"stopped",
 "lastAction":null}
```

`lastAction`, when present, is `{commandId:"uuid",result:"started"|"failed"}`.
Local desired is `running` or `stopped`. Publisher state is `created`, `ready`,
`starting`, `sending`, `reconnecting`, `stopping`, `stopped`, or `failed`.

Response contains all status fields plus
`serverRunId`, echoed `processEpoch`, `pollSeq`, `intentEpoch`, `nextPollMs:1000`,
and `action:null` or exactly:

```json
{"kind":"restartPublisher","commandId":"uuid","serverRunId":"engine-run-id",
 "controllerId":"uuid","processEpoch":"1","intentEpoch":"2","attempt":1}
```

## Safety/order rules

- Persist monotonic process/poll/intent high-water marks and intended state.
  Lower process, poll, or intent epochs are rejected. A higher process epoch
  starts stopped/unarmed; it must report a fresh manual running intent before
  actions are allowed. Same-epoch duplicate polls do not consume retry budget;
  redelivery uses the same command ID while still authorized by current policy.
- Controller ID is bound by enrollment. A new process increments a privately
  persisted process epoch under an OS file lock. Two OBS processes cannot share
  this local state concurrently. Server run ID fences replies across daemon restarts.
- Manual Start increments and persists local intent epoch/running **before**
  starting. Manual Stop increments/stops in memory, cancels pending replies, and
  persists stopped **before** stopping; persistence failure must still stop and
  disarm recovery. Graceful OBS EXIT stops/disarms. A new OBS process never resumes
  an old running file automatically.
- Plugin restarts only if reply controller/process/intent/source/server fences
  match its current state, policy enabled, no maintenance, source enabled, and
  local desired still running. Persist command ID before executing. Duplicates
  acknowledge the same result without a second restart. One HTTP request and
  one Publisher transition at a time. Reject redirects and non-HTTPS except loopback.
- Health comes from `MediaBridge::runtime_snapshot()` source
  `video_received_frames`, `audio_received_buffers`, `video_received_pts`, and
  continuity generation. Static pixels are healthy if these advance. Freeze means
  nonadvancing decoded media/PTS, not identical pixels or camera-scene content.
- Use a three-second progress grace/debounce, five-second controller lease,
  maximum three attempts, backoff 1/2/4 seconds, one in-flight command. Require
  two healthy post-restart observations to reset an incident. Persist attempt
  budget, in-flight command, and next eligibility across daemon/controller reconnect.
  Controller offline, disabled, maintenance, and local Stop cancel/withhold actions.
  A restart acknowledgement alone is not recovered usable video/audio.
- Operator disable/revoke invalidates pending future actions; status distinguishes
  offline/no acknowledgement. Local Stop always fences an already in-flight reply.

## Private bundle/local state

Keep publisher bundle `schemaVersion:1`; optional `recovery` is
`{schemaVersion:1,controlUrl,controllerId,controllerToken}`. CLI
`--with-recovery` explicitly enrolls; `--rotate-recovery` explicitly rotates.
CLI derives absolute control URL only from its validated explicit server origin
and returned source-specific relative path. Never copy operator passwords/cookies
into the bundle or OBS settings. No arbitrary URL supplied by a remote response.

Persist mode-0600 local controller state atomically beneath a private directory:
bundle path/source/controller ID, process epoch, intent epoch, desired state,
last command ID/result. Credentials remain only in the private scoped bundle.

## Acceptance and controls

Browser input configuration exposes enable, maintenance, status/receiver/attempts,
manual retry, and revoke. No remote Start button; explain that manual OBS Start
arms recovery and a dead process requires external supervision/manual intervention.
The OBS Tools dialog uses the module-owned Publisher runtime, shows recovery state,
and retains explicit Start/Stop. Loading configuration alone does not start.

Native bounded loopback acceptance must exercise real independent-output drop,
freeze with nonadvancing media, recovered video **and** audio, intentional Stop,
missing/wrong-source auth, exhausted backoff, controller reconnect/stale generations,
duplicate action delivery, maintenance/disable, and dead OBS reported offline.
Private fixture fault directives are finite allowlisted test operations, not a
generic command service. HTTP 200, output start signal, or declaration alone is
not recovery evidence. Root relays native runs to the same retained verifier.
