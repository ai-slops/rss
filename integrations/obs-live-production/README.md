# OBS Live Production Publisher

This out-of-tree OBS Studio 30.0.2 plugin publishes a selected OBS source or
scene through an independent H.264/AAC MPEG-TS SRT output. Video uses a
dedicated `obs_view`; audio uses one explicit OBS mix index from `0` through
`5`. The reusable publisher core accepts ordinary remote SRT targets. Only the
automated rehearsal fixture is restricted to loopback.

## Build and package

Build on the Ubuntu OBS host with the OBS 30.0.2 development packages and the
`obs-x264`, `obs-ffmpeg`, and `rtmp-services` modules available.

```sh
cmake -S integrations/obs-live-production \
  -B integrations/obs-live-production/build \
  -DCMAKE_BUILD_TYPE=RelWithDebInfo
cmake --build integrations/obs-live-production/build --parallel 2
ctest --test-dir integrations/obs-live-production/build --output-on-failure
cmake --install integrations/obs-live-production/build \
  --prefix integrations/obs-live-production/package
```

The install step directly stages the layout expected by the rehearsal:
`bin/64bit` and `data/obs-plugins/obs-live-production`.

## Publisher bundle

Each sender bundle follows `schemas/publisher-bundle.schema.json`. It contains
the scoped SRT passphrase and stream ID and must be a regular file with mode
`0600`. The plugin never includes the target host, port, URL, passphrase, or
stream ID in its report or explicit log messages.

The bundle's `bufferMs` is converted to FFmpeg's microsecond SRT `latency`
query value. For example, `120` becomes `latency=120000`.

Create a bundle for one input that already exists on the Live Production
server with the repository CLI. Keep the operator password in an owner-only
regular file; do not put it in the command line, URL, OBS profile or bundle.
The output path must not exist.

```sh
chmod 600 /secure/live-production/operator-password
node scripts/target/publisher-bundle.mjs \
  --server-url https://control.example.com/api/v1 \
  --password-file /secure/live-production/operator-password \
  --source-id 8d2f7b71-existing-source-id \
  --obs-source-name "Camera Scene" \
  --audio-mix-index 0 \
  --target-host ingest-host-visible-from-obs.example.com \
  --output /secure/obs/camera.publisher.json
```

Use the exact source ID from the server's authoritative snapshot. The command
does not create or select an input by name. It authenticates, verifies that the
requested persisted source is SRT, resolves its observed video/audio track IDs,
requests that source's scoped publishing endpoint, and writes one mode-`0600`
bundle. Its stdout is a credential-free JSON summary.

For a remote server, `--server-url` must use HTTPS and `--target-host` is
required. `--target-host` is the SRT host or IP reachable from the OBS machine;
it is not inferred from an HTTPS proxy. Loopback HTTP is accepted for a local
server, where omitting `--target-host` uses the loopback control hostname.
Encoding defaults are 1280x720, 30/1 fps and 2500 kbit/s; see `--help` for the
explicit override flags.

## Rehearsal fixture contract

`LIVE_PRODUCTION_OBS_FIXTURE_FILE` points to a private mode-`0600` JSON file:

```json
{
  "schemaVersion": 1,
  "localFixtureOnly": true,
  "senders": ["/private/red.publisher.json", "/private/blue.publisher.json"],
  "reportFile": "/private/obs-fixture-report.json",
  "controlFile": "/private/obs-fixture-control.json",
  "deadlineSeconds": 120
}
```

`senders` is an array of string paths, exactly matching
`scripts/target/harness/fixture-contract.mjs`. The private control file contains
a `commands` array defined by `schemas/control.schema.json`:

- `configureNativeFixtureSource` binds a sender bundle to a source ID/name,
  red or blue video, a changing binary frame counter, a sine tone, and one
  OBS audio mix.
- `startBadAuthSender` uses the harness bundle whose endpoint is identical to
  a primary sender except for its passphrase.
- `startDuplicateSender` reuses a primary bundle while that primary is still
  sending.

The plugin registers a hidden async OBS source type and emits actual BGRX video
at the bundle frame rate plus interleaved stereo float audio at 48 kHz. Each
source is routed only to its declared OBS mix with
`obs_source_set_audio_mixers`. The lower video band is a changing 16-bit frame
counter, so decoded one-frame-per-second samples visibly differ.

Primary senders must reach the OBS `start` signal before negative attempts are
started. A negative case is reported as `rejected` only after its output
actually fails to start, emits a non-success `stop`, or emits `reconnect`.
Merely declaring the expected outcome does not create evidence.

The report file is not created during setup. It is atomically written only
when both primary senders are still `sending` and both negative attempts were
observed rejected, or when a truthful failure/deadline result is available.
Its contract is `schemas/report.schema.json` and includes:

```json
{
  "schemaVersion": 1,
  "moduleLoaded": true,
  "module": "obs-live-production",
  "obsVersion": "30.0.2",
  "state": "ready",
  "senders": [{
    "sourceId": "...",
    "sourceName": "OBS Red 440",
    "audioMixIndex": 0,
    "state": "sending",
    "error": "",
    "visual": {"color": "red", "frameCounter": true},
    "audio": {"toneHz": 440}
  }],
  "negativeCases": [
    {"kind": "badAuth", "result": "rejected"},
    {"kind": "duplicateSender", "result": "rejected"}
  ]
}
```

## Production frontend integration

The fixture environment variable is not the production control surface. In a
normal OBS launch the plugin adds **Tools → Live Production SRT Publisher**.
The native Qt dialog uses OBS system styling and owns one independent
`Publisher` for the lifetime of the module.

Operator workflow:

1. Provision one input on the Live Production server, then run the command in
   **Publisher bundle** above for that existing source ID. Keep the generated
   file private and do not print or copy its contents into reports.
2. In OBS, create or select the video source or scene named by the bundle and
   route the intended stereo audio to the bundle's OBS mix index.
3. Open **Tools → Live Production SRT Publisher** and select the bundle. The
   dialog displays only its file path, selected OBS source, stereo mix and
   encoding summary; target credentials are never displayed.
4. Confirm the displayed source and mix. **Start publisher** is enabled only
   after this explicit confirmation. It starts the plugin's dedicated SRT
   output, not OBS's main Stream or Record controls.
5. Observe the textual state (`Starting`, `Sending media`, `Reconnecting`,
   `Stopping`, `Stopped` or `Failed`) and any local error. Use **Stop
   publisher** explicitly. Closing the dialog leaves an active publisher
   running so it can be reopened from Tools; closing OBS stops plugin-owned
   publishing.

The selected path and bundle contents are held only in plugin memory. They are
not saved in an OBS profile, scene collection, plugin settings file or report,
and the plugin does not log the path, target host, port, passphrase or stream
ID. Nothing auto-starts after OBS restart or profile load. When
`LIVE_PRODUCTION_OBS_FIXTURE_FILE` is set, the Tools action is intentionally
not installed, preventing manual publishers from colliding with the automated
fixture.

Ubuntu GUI validation for OBS 30.0.2:

1. Build and install the plugin with Qt 6 Widgets, `libobs` and
   `obs-frontend-api` development packages.
2. Launch OBS from a terminal without `LIVE_PRODUCTION_OBS_FIXTURE_FILE` and
   confirm the Tools item appears after module load.
3. Select a valid private bundle and verify the displayed source, mix and
   encoding match the bundle. Verify an invalid or overly permissive file
   disables Start and reports the validation error without revealing secrets.
4. Start and stop while observing the server's scoped input. Confirm OBS's main
   stream remains stopped, only the selected source and one stereo mix arrive,
   and reopening the dialog preserves the current in-memory state.
5. Restart OBS and confirm the publisher is stopped and no bundle path or
   credential was restored.

Native compilation, module loading, real SRT admission, decoded frame-counter
motion, tone dominance, and negative-case timing remain Ubuntu-only checks.
The Alpine workspace can validate schemas and source contracts but cannot make
those native claims.
