# OBS source recovery runtime

The module owns one independent `PublisherRuntime`. The Tools dialog and source
recovery controller use that same runtime; closing the dialog does not destroy
the publisher. Recovery never invokes OBS main Stream/Record, scene changes,
capture-device commands, or process restart.

## Bundle and intent

Publisher schema version 1 accepts optional:

```json
{"recovery":{"schemaVersion":1,"controlUrl":"https://server/api/v1/source-recovery/source-id/poll","controllerId":"uuid","controllerToken":"private-source-token"}}
```

The bundle remains mode `0600`. The token is used only as a bearer credential
for the exact source poll URL. Redirects are rejected. HTTP is accepted only
for `localhost`, `127.0.0.1`, or `::1`; all other control URLs require HTTPS.

Loading a bundle configures polling but always starts stopped. A new OBS
process increments `processEpoch`, persists stopped intent, and never resumes
an old running file. If the operator presses Start before the first controller
response has registered this process as stopped, that explicit gesture is
queued in memory and executed only after the matching stopped response. It is
never restored after process restart. Manual Start then increments and persists
running intent before starting. Manual Stop changes the in-memory fence, cancels the in-flight HTTP
request, persists stopped, and stops even if persistence fails.

State is atomically stored under the OBS module config directory in a private
mode-`0700` directory with mode-`0600` files and a nonblocking OS file lock.
Stored fields are bundle/source/controller identity, process/poll/intent epochs,
desired state, and last command/result. The controller token remains only in
the private bundle.

## Poll and action behavior

The Qt Network client has one one-shot timer and at most one request in flight.
It sends the contract-v1 JSON with decimal-string epochs and publisher state.
Replies and actions must match source ID, controller ID, server run ID,
process/poll/intent epochs, policy enabled, maintenance false, source enabled,
and local desired running.

`restartPublisher` persists the command ID before destroying and recreating
only the independent publisher/view/encoders. The result is persisted and sent
as `lastAction`. Redelivery of a completed command returns the same result and
does not restart twice. Publisher start is not treated as recovered media; the
server owns decoded video/audio progress and retry recovery decisions.

If a newer valid status reports policy disabled, maintenance, source disabled,
controller offline, or a different server run while an action is held, the
module closes the action receiver, persists that command as `failed`, and
clears the pending action. Network/authentication errors do the same. This
withholds recovery but does not stop an already-running manually started
publisher. A later eligible command uses a fresh dispatcher/action.

## Native finite fault fixture

Set `LIVE_PRODUCTION_OBS_RECOVERY_FIXTURE_FILE` to a private file matching
`schemas/recovery-fixture.schema.json`. The current harness writes:

```json
{"schemaVersion":1,"localFixtureOnly":true,"sourceId":"id","publisherBundle":"/private/bundle.json","reportFile":"/private/obs-recovery-report.json","commandFile":"/private/obs-recovery-command.json","deadlineSeconds":180,"operations":[{"kind":"dropPublisher"},{"kind":"freezePublisher","durationMs":4000},{"kind":"manualStopAfterQueuedRestart"},{"kind":"pollMissingAuthorization"},{"kind":"pollWrongSourceAuthorization"},{"kind":"exhaustRetryBudget"},{"kind":"reconnectController"},{"kind":"redeliverDuplicateAction"},{"kind":"observeServerRestart"},{"kind":"setMaintenance"},{"kind":"setSourceDisabled"},{"kind":"letControllerLeaseExpire"}]}
```

The fixture creates a real hidden red/frame-counter/440 Hz OBS source, routes
its stereo tone to the bundle mix, waits for the first stopped registration
response, and then performs its explicit initial manual Start. It does not run
the baseline fixture publisher on the same endpoint.

The harness writes one mode-`0600`, regular, non-symlink command file at a time using
`schemas/recovery-command.schema.json`, for example:

```json
{"schemaVersion":1,"commandId":"4","kind":"dropPublisher"}
```

Command IDs are strictly increasing. The plugin executes only kinds present in
the fixture allowlist and acknowledges each command in the report with its ID,
kind and result. The report also includes process/intent/poll epochs, last
server recovery command/result, `recoveryActionPending`,
`pendingRecoveryCommandId`, `duplicateCommandId`, `duplicateResult`, and a
nonsecret restart execution count. `duplicateResult` is `notObserved` unless a
retained real action was presented to the actual handler and matched its
persisted completed command/result. This
lets the harness wait for decoded media or controller state before issuing the
next fault instead of relying on fixed delays.

For the queued-Stop race, the fixture allowlist must include
`pauseActionDispatch` and `resumeActionDispatch`. The harness pauses the actual
module action dispatcher, drops the independent publisher, waits until the
report exposes `recoveryActionPending:true` and a nonempty
`pendingRecoveryCommandId`, issues `manualStopAfterQueuedRestart`, then resumes.
The same report must show the pending ID cleared and `restartExecutionCount`
unchanged.

`dropPublisher`
stops only the independent output without changing running intent.
`exhaustRetryBudget` arms three real restart failures. Controller reconnect and
lease-expiry operations pause outgoing polls and later resume them. Manual Stop
uses the normal intent fence. Missing/wrong authorization, duplicate delivery,
server restart, maintenance, and source-disable are server-directed operations;
the plugin report labels them `serverDirected` rather than inventing outcomes.

`pollMissingAuthorization`, `pollWrongSourceAuthorization`, server restart,
maintenance and source-disable are labelled `awaitingHttpEvidence`; they are
not reported verified until the harness supplies actual HTTP/server evidence.

`freezePublisher` gates the independent Publisher's module-owned in-process
MPEG-TS/SRT writer through that exact output's procedure handler. It finishes
in-flight mux/AVIO work and confirms no send is active within 500 ms before
acknowledging `executed`. Timeout reports `publisherTransportNotQuiescent`.
Packets and resume operations are epoch-fenced; frozen admission is bounded and
resume waits for an IDR. Qt/controller processing and other publishers remain
live. A finite instance-fenced timer resumes within at most 30 seconds; manual
Stop quiesces, interrupts and joins only its own writer. No child process,
arbitrary thread, source mute or global packet interposition is involved.

This is independent of the harness's network bridge DATA pause. The verifier
must observe stalled media, online controller, recreation/new generation and
strictly decoded recovery on actual OBS; neither the hook acknowledgement nor
the standalone ownership regression proves the entire recovery workflow.

Whole-process `SIGSTOP` is a separate offline boundary: the frozen OBS process
cannot poll or recover itself and must remain reported offline until the harness
resumes it. The finite report records invoked directives and current
publisher/local-intent/receiver states; it is not itself proof of recovered A/V.

## Native verification

Build with Qt 6 Network/Widgets, libobs and obs-frontend-api, then run CTest.
Native acceptance must retain the private recovery fixture/report, OBS log,
controller HTTP receipts, and decoded receiver video/audio counters. A pass
requires real advancing decoded media after publisher recreation and truthful
manual Stop, stale action, duplicate, maintenance, disable, lease and exhausted
budget behavior. Process death remains offline until external supervision or a
manual relaunch.
