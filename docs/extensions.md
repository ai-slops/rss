# Trusted extensions

The first extension boundary is intentionally small. Extensions are trusted Rust modules registered at coordinator startup; there is no marketplace, hot reload, process sandbox, raw-frame JSON bus, or universal plugin ABI.

## Registered examples

The authoritative snapshot lists registered modules in `plugins`, including each trusted `storageNamespace`, typed command metadata, and optional panel descriptor:

- `editor-session` journals production observations and exports an editor package. It is available but disabled until a session starts.
- `external-event-panel` demonstrates an external-event panel without adding platform, chat, donation, viewer, or game data to the production domain.

Both use the normal authenticated `POST /api/v1/commands` route and `plugin.command` envelope. `targetId` selects the registered plugin. Unknown plugins and commands are rejected without mutating production state.

Bundled media adapters also use trusted typed factory registrations rather than a second hidden dispatch path. `SOURCE_FACTORY_KINDS` registers synthetic and SRT source factories; `OUTPUT_FACTORY_KINDS` registers Program recording, source recording, and RTMPS output factories. Normal `source.create` and `output.create`, including restored output registration, invoke those callbacks. These are build-time Rust callbacks, not a universal ABI or hot-load mechanism.

```json
{
  "id": "external-event-1",
  "kind": "plugin.command",
  "targetId": "external-event-panel",
  "expectedEngineRunId": "<engine-run-id>",
  "payload": {
    "command": "event.record",
    "text": "Goal"
  }
}
```

The example panel stores its own event count and latest text in its namespaced storage. The command receipt exposes only the applied count in `pluginResult`; complete external-event history is not copied into core production events.

## Editor session commands

| Command | `text` | Result |
|---|---|---|
| `session.start` | Required session ID | Starts a fresh journal from a consistent snapshot/cursor. |
| `marker.add` | Required marker label | Adds an editorial marker at server time. |
| `session.export` | Omitted | Returns the editor package in `pluginResult`. |
| `session.stop` | Omitted | Stops journaling; core production and outputs remain untouched. |

The export contains the recording inventory and paths, clean-source/Program tap classification, Program reference, applied layout/source-use intervals, audio history, timing history, markers, and known recording/observation gaps. Program observations include the exact pinned `appliedScene`, not a later draft with the same scene ID. Media application times are labeled `engineRunPtsNs`; marker times are labeled Unix wall time. Until runtime and recorder observations provide a bounded mapping between those clocks, recording intervals remain `unknown`, `timebase.mapping` is `null`, reconstruction is `approximate`, and issues include `missingClockMapping` plus `unknownRecordingTimebase` when media exists. The package must not convert these values by subtraction. If no Program recording is present, `programReference` is `null` and issues also contain `{"kind":"missingProgramReference"}`. The plugin never starts or stops an output implicitly.

Unknown plugin configuration remains in SQLite namespaced storage when a module is absent. Disabling or stopping the editor session only releases editor-owned journal work; it cannot stop broadcasting or recording.

## Reliable observation

`GET /api/v1/snapshot` is serialized through the coordinator and includes the exact durable event `cursor` current for that state. Reconnect with:

```text
GET /api/v1/events?after=<snapshot-cursor>
```

The server subscribes before replaying durable events, so commands committed between snapshot and live delivery are recovered. Each SSE frame has:

- `id`: durable cursor
- `event`: `production.applied`, `production.rejected`, or `production.failed`
- `data`: the stored event envelope, including the receipt and authoritative post-command snapshot

The retained journal is bounded. If `after` predates the oldest retained event, the stream first emits `event: gap` with `requestedAfter` and `oldestAvailable`; clients must fetch a fresh snapshot and resume from its cursor. Broadcast lag triggers durable replay rather than anonymous invalidation.

## Verification

Run the focused proof with the project toolchain:

```sh
scripts/test-extensions.sh
```

The tests cover trusted registration, public external-event dispatch, missing Program-reference reporting, marker/layout/audio/timing/gap export fields, and authoritative applied snapshots after a cursor.
