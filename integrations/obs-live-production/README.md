# OBS Live Production Publisher

Use **Tools → Live Production SRT Publisher → Send all individual OBS sources**
with a private server connection file to register and publish every current OBS
input separately. Cameras, captures, media and standalone audio remain separate
server inputs; OBS scenes and groups are not flattened into one stream. Each input
gets an independent video view, encoder, SRT process and source-audio capture. OBS
source mute is respected. A first grid layout is prepared in the web studio; use
Take there to apply it. Later starts preserve web layout edits. Stop all and start
again to discover inputs added after publishing began.

The connection JSON contains `schemaVersion: 1`, `serverUrl` (HTTPS),
`sourceHost`, `caCertificatePem` and `password` (the operator credential). Treat it
as a private credential file. Source UUID bindings are stored per user without
credentials. Forward UDP 9000–9015 to the server; each source needs one free port.
Windows packages include the Qt Schannel TLS backend alongside the plugin.

The manual single-source mode publishes a selected OBS source or
scene through an independent H.264/AAC MPEG-TS SRT output. Video uses a
dedicated `obs_view`; audio uses one explicit OBS mix index from `0` through
`5`. The reusable publisher core accepts ordinary remote SRT targets. Only the
automated rehearsal fixture is restricted to loopback.

## Build and package

Build on the Ubuntu OBS host with the OBS 30.0.2 development packages and the
`obs-x264` and `obs-ffmpeg` encoder modules available. The module-owned
MPEG-TS/SRT output also needs native FFmpeg format/codec/util and libsrt
development packages discoverable through pkg-config. It does not require a
custom RTMP service or a patched system OBS output backend.

On Ubuntu 24.04 use `libavformat-dev libavcodec-dev libavutil-dev
libsrt-gnutls-dev libgnutls28-dev nettle-dev`.

The plugin owns a small `obs-live-production-srt` executable alongside its module.
It isolates libsrt's process-global state from OBS's FFmpeg module and receives
credentials and MPEG-TS only through private IPC. Linux uses a socket and Windows
uses inherited pipes with length framing and a kill-on-close Job Object. Each send is acknowledged
after libsrt accepts it. Stop closes IPC, then terminates and reaps a
blocked helper within a bounded grace period; parent death also stops it.
Install both binaries together using the package below.

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

For an ordinary Linux OBS installation, package that stage and extract it into
the current user's OBS plugin directory while OBS is closed:

```sh
bash scripts/package-obs-plugin.sh integrations/obs-live-production/package \
  /tmp/obs-live-production-linux.tar.gz
mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/obs-studio/plugins"
tar -xzf /tmp/obs-live-production-linux.tar.gz \
  -C "${XDG_CONFIG_HOME:-$HOME/.config}/obs-studio/plugins"
```

Start OBS and open **Tools → Live Production SRT Publisher**. Build against
the OBS/libobs and Qt versions installed on the sender. Linux `.so` packages
cannot be installed into Windows or macOS OBS, or into a Flatpak sandbox as
ordinary native packages.

### Windows x64 / OBS 33

The Windows package targets **OBS Studio 33.0.1 x64**, with its Qt 6.11 and FFmpeg
62 runtime. Use the matching OBS version; the Linux archive and older OBS ABI
packages are not interchangeable.

Close OBS, run the Windows setup executable and restart ordinary installed OBS.
The installer places the module, SRT helper and libsrt DLL under
`%ProgramData%\obs-studio\plugins\obs-live-production\bin\64bit`, with resources
under the sibling `data` directory. Remove it through Windows Installed Apps.
Portable OBS deliberately searches its own plugin locations instead of the
system plugin directory, so this system installer is for ordinary OBS mode.

Open **Tools → Live Production SRT Publisher**, select the private JSON bundle,
confirm the named source/scene and stereo mix, and click **Start publisher**.
The source name in the bundle must match an existing OBS source or scene exactly.
Audio must be 48 kHz stereo. Close does not stop the publisher; use **Stop publisher**.

To build from source, install Visual Studio 2022 C++ Build Tools with the Windows
SDK and CMake, then run on Windows x64:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/windows-obs-build.ps1
```

`-CMake` can select the full `cmake.exe` path. The script verifies fixed official
OBS/dependency SHA-256 hashes, builds the OBS development libraries and plugin,
runs Windows ACL/exclusive-owner/atomic-recovery-state tests, and writes the ZIP
under `var/windows-build`. Build logs belong in the CI/job output.

To create the setup executable from that Windows-built package on Linux:

```sh
bash scripts/package-obs-windows.sh /path/to/package/obs-live-production \
  /tmp/obs-live-production-obs33-windows-x64-setup.exe
```

The packaging script builds a local NSIS container when needed. Installation
packages contain no server operator password or scoped publishing credentials.

CTest also includes a real libobs audio-root regression. It initializes a small
software-rendered video context, emits 440/880 Hz on separate mixes and requires
absolute RMS/tone amplitude with negligible cross-mix leakage. It uses `DISPLAY`
when supplied or executes through `xvfb-run -a`; install Xvfb/xauth on a headless
test host. No physical audio device is required. Private fixture sources claim
only unused audio-root channels above channel 0 and remove their exact roots on
teardown; an auxiliary video view alone is not part of OBS's global audio mix.

## Publisher bundle

Each sender bundle follows `schemas/publisher-bundle.schema.json`. It contains
the scoped SRT passphrase and stream ID and must be a regular file with mode
`0600` on Linux. On Windows, the file must have a private DACL granting access
only to the current user, SYSTEM and Administrators, with no reparse point.
Store downloaded bundles in your private user profile. The plugin never includes the target host, port, URL, passphrase, or
stream ID in its report or explicit log messages.

The bundle's `bufferMs` is applied directly to libsrt's millisecond latency
option. For example, `120` configures 120 ms. The owned backend does not assemble
a credential-bearing FFmpeg URL.

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

### Optional source recovery enrollment

Recovery is never enrolled implicitly. Add `--with-recovery` to the same bundle
command only after recovery policy for that existing SRT source is available:

```sh
node scripts/target/publisher-bundle.mjs \
  --server-url https://control.example.com/api/v1 \
  --password-file /secure/live-production/operator-password \
  --source-id 8d2f7b71-existing-source-id \
  --obs-source-name "Camera Scene" \
  --audio-mix-index 0 \
  --target-host ingest-host-visible-from-obs.example.com \
  --output /secure/obs/camera-with-recovery.publisher.json \
  --with-recovery
```

The operator cookie is used only by this enrollment command. The bundle receives
one source-specific controller ID/token and a control URL constructed from the
validated server origin plus the server's exact relative path for that same
source. The CLI rejects redirects, absolute response URLs, another source's
path, and generic command paths. The operator password/cookie is never copied
into the bundle or passed to OBS.

An existing enrollment is not silently replaced. To intentionally invalidate
the old source controller token and issue a new one, choose a new output path
and use both flags:

```sh
node scripts/target/publisher-bundle.mjs ... \
  --output /secure/obs/camera-rotated.publisher.json \
  --with-recovery --rotate-recovery
```

`--rotate-recovery` without `--with-recovery` is rejected. Loading a recovery
bundle does not start publishing. Manual **Start publisher** must establish a
fresh running intent for the current OBS process; manual Stop and OBS exit
disarm recovery.

### Recovery acceptance entrypoints

The portable local driver uses a fake source controller and real supervised
GStreamer SRT sender/receiver. It proves controller fencing and decoded A/V, but
is explicitly not OBS evidence:

```sh
source scripts/env.sh
LIVE_PRODUCTION_RECOVERY_DRIVER=1 \
LIVE_PRODUCTION_RECOVERY_ROOT="$PWD" \
LIVE_PRODUCTION_RECOVERY_SUPERVISOR="$PWD/target/debug/live-production-supervisor" \
node --test scripts/target/recovery-local.driver.test.mjs
```

When the recovery backend is built locally, a second driver exercises the real
operator/controller HTTP routes and native receiver counters without starting
outputs or changing Program:

```sh
source scripts/env.sh
LIVE_PRODUCTION_RECOVERY_HTTP_DRIVER=1 \
LIVE_PRODUCTION_RECOVERY_ROOT="$PWD" \
LIVE_PRODUCTION_RECOVERY_SUPERVISOR="$PWD/target/debug/live-production-supervisor" \
LIVE_PRODUCTION_RECOVERY_SERVER="$PWD/target/debug/live-production" \
LIVE_PRODUCTION_RECOVERY_WEB_DIST="$PWD/web/dist" \
node --test scripts/target/recovery-http.driver.test.mjs
```

This remains non-OBS evidence. The real-server driver destroys and recreates a
supervised synthetic SRT sender through an explicitly authorized loopback bridge
handover. It requires a new continuity generation, two strict video/audio/PTS
observations, and decoded red-motion/440 Hz stereo recordings. It does not claim
OBS Publisher recreation. A newer manual stopped intent cancels the next queued
restart. Actual OBS destruction/recreation belongs to the native fixture below.

Native preparation writes a private finite fault file for drop, freeze, manual
Stop, authorization, backoff, reconnect, duplicate delivery, server restart,
maintenance, source-disable and offline-lease cases:

```sh
scripts/target/recovery-native.sh \
  --supervisor-binary "$PWD/target/release/live-production-supervisor" \
  --server-binary "$PWD/target/release/live-production" \
  --web-dist "$PWD/web/dist" \
  --plugin-dir "$PWD/integrations/obs-live-production/package" \
  --obs-binary /usr/bin/obs \
  --xvfb-binary /usr/bin/Xvfb \
  --evidence-dir "$PWD/var/recovery-native-$(date +%s)" \
  --deadline-seconds 300
```

The entrypoint starts an isolated server, creates one loopback SRT input,
enables recovery using the freshly observed policy revision, enrolls a one-time source controller, writes the
private publisher bundle/fault/command files, and supervises Xvfb and OBS. Each
strictly increasing private command is acknowledged through the plugin report's
`executedCommand*` fields. The harness independently verifies backend HTTP
evidence, receiver counters/PTS, restart execution count, manual Stop fencing,
authorization, backoff, duplicate delivery, server restart, maintenance,
source-disable and lease behavior.

For online media freezes, the harness places an owned loopback-only UDP bridge
between the OBS bundle port and the real server SRT port. It forwards SRT
control datagrams in both directions while selectively dropping only DATA
datagrams from the single fixture peer. No payload is decoded or logged by the
bridge. Publisher recreation requires an explicit one-shot handover: only a new
control handshake is accepted, it receives a distinct loopback server-facing
socket, old DATA is rejected, and the old control route is retained until the
harness observes the new decoded continuity generation and retires it.
Maintenance and source-disable cases must reach a real stalled/disabled
receiver condition with the controller online and no additional recovery action.
Whole-process `SIGSTOP` remains a separate offline boundary. Queued Stop proof
uses the plugin's private action-dispatch pause/resume hooks and requires a
nonempty pending recovery command ID before Stop, then no execution after the
dispatcher resumes.

The entrypoint validates the plugin-owned `RECOVERY.md` and implementation for
`commandFile`, `restartExecutionCount`, and command acknowledgments before
launch. Missing local OBS/Xvfb/Qt/plugin staging is reported as a native
prerequisite failure, not as a missing recovery contract. A native pass requires
actual OBS source events plus advancing decoded receiver video and audio after
publisher recreation; HTTP 200, output start, or a declared expected result is
insufficient.

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

Ordinary bundles and their selected path are held only in plugin memory. Opt-in
recovery additionally persists the bundle path and nonsecret intent/epoch state
in a separate private module state directory; scoped credentials remain only in
the private bundle. Credentials are not saved in an OBS profile, scene collection,
or report, and the plugin does not log the path, target host, port, passphrase or
stream ID. Nothing auto-starts after OBS restart or profile load. When
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
