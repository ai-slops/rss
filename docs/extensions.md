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

The export contains the recording inventory and paths, clean-source/Program tap classification, Program reference, applied layout/source-use intervals, audio history, timing history, markers, and known recording/observation gaps. Program observations include the exact pinned `appliedScene`, not a later draft with the same scene ID. Media application times are labeled `engineRunPtsNs`; marker times are labeled Unix wall time. The plugin never starts or stops an output implicitly.

The editor journal now retains bounded engine-media/wall-clock anchors. New recorder catalogs include engine-run identity and distinct per-file media bounds from actual H.264/AAC packet timestamps. Recorder origin sampling uses the recording pipeline's actual running time after PLAYING, not the duration or midpoint of its state-change call. Engine-run-scoped native reference metadata measures producer-video PTS against recorder PTS across the inter-pipeline bridge. The observed offset range and maximum packet duration contribute to the declared uncertainty. This is an approximate, video-referenced server-media mapping, not proof of original camera capture time, isolated audio alignment, or long-run drift correction.

For a matching engine run, `timebase.mapping` supplies the captured clock anchor and each mapped inventory item has `intervalClockDomain: "engineRunPtsNs"` plus `fileTimeMapping` containing media/wall bounds and uncertainty. Earlier catalogs/journals remain readable with missing mapping fields. Missing clocks, unmatched runs, or unavailable bounds remain explicitly unknown and produce `missingClockMapping` or `unknownRecordingTimebase`; unrelated wall and media numbers are never directly subtracted. Reconstruction remains `approximate`. If no Program recording is present, `programReference` is `null` and issues contain `{"kind":"missingProgramReference"}`.

New clean recordings capture stable source/track IDs and per-track continuity generation intervals in native reference metadata. Only a completed owner-thread flush authorizes a reconnect epoch; old queued media retains its old generation. Each fragment clips captured intervals and availability gaps proportionally between local/media endpoints, persists them in session/catalog metadata and exports them after reload. A mixed-generation file keeps every interval; its single `sourceContinuityGeneration` summary may be null because no single generation describes it. Legacy files without captured provenance still report `unknownSourceGeneration` instead of borrowing the current connection. All new nanosecond JSON values are decimal strings, with legacy numeric catalog reading supported.

Startup discovery includes raw files missing from catalogs and interrupted/partial tails. Inventory/export exposes Recovery, Validation, Mapping and interruption gaps independently; it does not delete raw files or invent a new engine clock. Actual SIGKILL, queued red/440 Hz to blue/880 Hz reconnect, precision and catalog-reload evidence are in `docs/internal-gates-2026-10-27.md`.

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

The tests cover trusted registration, public external-event dispatch, missing Program-reference reporting, legacy journal/catalog compatibility, engine-run mismatch rejection, per-file mappings, marker/layout/audio/timing/gap fields, and authoritative applied snapshots after a cursor. `media/tests/recording_mapping_contract.rs` decodes real green/red Program transitions in different Matroska fragments and checks their mapped PTS against authoritative Take PTS within exported uncertainty plus one frame. The measured focused run differed by at most about `0.4 ms`, with about `33.34 ms` declared uncertainty; this is a controlled fixture result, not a universal performance claim.
