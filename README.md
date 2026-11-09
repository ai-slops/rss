# Live Production

Headless Ubuntu live production operated from a browser.

Implementation baseline: [handoff](docs/implementation-handoff.md).

## Development

Run verification against the source and target environment you intend to use. Test results belong in CI logs and ignored `var/` artifacts, rather than committed execution receipts.

For project-local Alpine/musl development dependencies, run:

```sh
./scripts/dev.sh
```

The script provisions project-local dependencies if absent, installs locked UI dependencies, builds both applications, and starts the daemon at `http://127.0.0.1:3000`. No server GUI or VM is needed. Allow several minutes for the first dependency/build pass. Rust builds default to two jobs; the assigned budget is eight CPUs, 32 GiB and 8192 PIDs.

The generated operator password is in `var/operator-password` (mode `0600`); the state directory is `0700`. Read it privately to sign in. Never paste it into issue reports, URLs, screenshots, source files or shell commands recorded as evidence. Alternatively provide a strong `LIVE_PRODUCTION_PASSWORD` through the deployment's secret environment. Credentials under 16 bytes are rejected. The browser receives an HttpOnly, SameSite=Strict session cookie, not the password or platform credentials. Daemon restart invalidates sessions.

For HTTPS behind the workspace's approved shared ingress:

```sh
LIVE_PRODUCTION_SECURE_COOKIE=true ./scripts/dev.sh
dim-expose --name studio --port 3000 --ingress shared
```

Verify the reported HTTPS URL and sign-in before reporting it usable. Control HTTPS does not prove WebRTC ICE connectivity. Do not expose an unauthenticated GStreamer signaling server, open host ports or broadcast publicly for testing.

## Engineering checks

Native preview/test process ownership and the repeated-run regression are documented in [lifecycle and cleanup](docs/lifecycle.md). All supplied runners supervise their own process trees; do not replace them with background `cargo run`, `nohup` or `setsid` launches.

```sh
./scripts/acceptance.sh --quick  # strict Rust/UI checks and tests
./scripts/acceptance.sh          # also actual media, network, browser and extension gates
```

Full acceptance fails if a required gate is absent. Reports and generated fixtures stay under `var/`; the latest command report is `var/acceptance/latest.txt`. [Media architecture](docs/stage-a.md) describes the media boundary and test entrypoints. A software fixture is not proof of hardware 1080p60/4K capacity, OBS track separation, target-laptop ICE, platform semantics or editor interoperability.

Workspace Rust tests include a three-minute native drift/timing/recording regression even with `--quick`; that flag skips standalone CLI/browser gates, not the Rust endurance contract. Full acceptance additionally requires an actual supervised SIGKILL/restart recording-recovery gate. When a studio must stay running, validate in a separate checkout/target and separate web output directory: the normal development build must not replace its executable or served bundle implicitly.

## Dependencies and Ubuntu deployment

Rust and npm dependencies are locked in `Cargo.lock` and `web/package-lock.json`.
The Ubuntu verification scripts select Rust/Cargo 1.91.1 and native libraries.
For project-local Alpine dependencies, `scripts/bootstrap.sh` installs under
`.tools/` and `scripts/env.sh` activates them; see the
[local-musl ADR](docs/adr-001-local-musl-toolchain.md).

On Ubuntu, have the deployment administrator supply Rust/Cargo, a C compiler, pkg-config, SQLite build prerequisites, FFmpeg, GStreamer development libraries and the base/good/bad/ugly/libav plugin sets. Required capabilities include compositor, audiomixer, H.264 CPU encoding, AAC, MPEG-TS, Matroska/splitmux, SRT and RTMPS; WebRTC additionally needs the Rust rswebrtc plugin and working ICE dependencies. Use native Ubuntu packages/libraries, not the Alpine sysroot. Build locked dependencies and rerun acceptance on the actual target before relying on performance. Hardware encoding is not assumed.

## OBS publisher profile

Provision a named authenticated input first. OBS may publish an ordinary network stream; device enrollment, Tailscale, a desktop companion and platform account credentials are not required. The baseline transport profile is SRT carrying H.264 video and AAC audio in MPEG-TS, with a bounded transport latency configured separately from manual sync correction and Program holdback. Use the source's scoped ingest credential, never an operator or platform credential.

The authenticated publishing endpoint supplies both the scoped passphrase and `streamId`; configure the sender's SRT `streamid` option from that value. Do not log or export these secrets. Native admission is peer-specific: simultaneous publishers cannot take another active/pending lease, and failed handshakes are recovered by closing the old listener before reuse. Legacy empty-streamid clients retain the non-reassigning lease behavior, but new provisioning should use the supplied value. Real OBS/plugin interoperability remains unverified until exercised on that sender.

An incoming mixed stereo track remains mixed stereo; do not advertise isolated stems unless the actual sender/plugin exposes and the receiver observes them. Real OBS/plugin track separation has not been tested in this environment. Unknown and duplicate publishers must not replace an active source. Consult the final input evidence before relying on the provisioning path.

## Production semantics

- Scene edits are Draft changes. Selecting/cueing never publishes; Take uses the exact committed scene revision and current engine run/Program revision. Preview stays selected after Take. Cue previous Program changes Preview only.
- Audio routes, gain/mute and source timing are Live/shared, independent of scene visibility. New audio is unrouted until explicitly applied. A break scene does not mute microphones.
- Listen, monitor volume/mute and panel selection are This browser controls. Monitoring starts muted. Media-disconnected and control-disconnected are different states.
- Every input has an automatic clean-source recorder, armed before media arrives and resumed after daemon restart. It saves video and audio before Program timing, composition, gain and mute, in 60-second MKV segments. Disconnects finalize the current session; reconnects open a new one. Program recording remains optional. Recording and each network destination have independent desired/actual states. Start sending may immediately create a public broadcast. Configuration reload and daemon restart are not authorization to restart public output.
- Live commands are not queued offline. Reconnect requires a fresh authoritative snapshot; stale drafts are rejected rather than overwriting another browser's work.

## Storage, recovery and operation

`var/project.sqlite` stores configuration, command receipts and observations; ordinary media/assets/journals live beneath the state directory. `.tools/`, `target/`, `web/node_modules/`, build artifacts and `var/` are ignored by Git. Keep backups outside a running capture interval or use SQLite's supported backup procedure; do not copy a live WAL database as though a single file were a consistent backup. Never delete recordings automatically to free space.

Environment configuration:

| Variable | Default | Meaning |
|---|---|---|
| `LIVE_PRODUCTION_BIND` | `127.0.0.1:3000` | Workspace-loopback HTTP listener |
| `LIVE_PRODUCTION_STATE_DIR` | `var/` | Private persistent configuration/media root |
| `LIVE_PRODUCTION_WEB_DIST` | `web/dist/` | Built UI directory |
| `LIVE_PRODUCTION_PASSWORD` | generated private file | Operator credential; at least 16 bytes |
| `LIVE_PRODUCTION_SECURE_COOKIE` | daemon `true`, dev script `false` | Set `true` behind HTTPS ingress |
| `CARGO_BUILD_JOBS` | `2` in project scripts | Build concurrency |

Run the daemon as an unprivileged service account with write access only to its state directory and the required media devices. The executable runs without a browser; closing the browser must not stop media. Send SIGTERM for graceful shutdown and recording finalization. A native in-process media/plugin crash can terminate the daemon; trusted registration is not a sandbox. Crash-damaged segments and gaps must remain visible, and restart recovery must not silently mark them decode-validated.

Recorders now persist session metadata before capture and publish only confirmed closed fragments. Startup inventories raw MKVs even without a valid catalog, preserves raw bytes, and reports recovered/partial, decode validation, mapping and interruption gaps separately. Unknown wall/engine timing is labelled unknown. Clean-source generation intervals come from captured video/audio metadata, not the current connection. Automatic source recorders resume after restart; manual Program/source recorders and network outputs remain stopped. Recovery never authorizes a public transmission. Source archives are stored beneath `<state directory>/recordings/<output ID>/`; files are retained when an input is removed and are never automatically deleted to free space.

An Ubuntu service example is provided in `deploy/live-production.service`. It assumes a pre-created unprivileged account, a release build under `/opt/live-production`, native deployment dependencies and an authenticated HTTPS reverse proxy. It is not installed automatically, and the Ubuntu service/hardware profile remains unverified until exercised there.

For a container with HTTPS, persistent state and router-forwarded SRT/WebRTC UDP,
see the [container deployment instructions](deploy/README.md). Sender access does
not require Tailscale.

## Native OBS and VM verification

The OBS plugin's build, installation and publisher workflow are documented in
[integrations/obs-live-production](integrations/obs-live-production/README.md).
The server needs a UDP SRT endpoint reachable from the sender in addition to its
HTTPS control URL.

On a dedicated Ubuntu 24.04 host with Rust 1.91.1 installed:

```sh
bash scripts/ubuntu-dependencies.sh
bash scripts/ubuntu-obs-check.sh
```

The [Native OBS workflow](.github/workflows/native-obs.yml) runs the same checks
when manually started in GitHub Actions.

For a dedicated KVM guest on Linux, install QEMU, cloud-image-utils, OpenSSH,
Node and curl, then run:

```sh
bash scripts/ubuntu-verify.sh run
```

The runner downloads and checks an official Ubuntu cloud image, snapshots the
current working source (including tracked edits), and owns the VM until completion.
It builds the plugin, runs real OBS SRT/decoded-audio/video and recovery checks,
and runs the software checks. Logs and credentials remain under ignored `var/`;
the VM is stopped on completion or interruption. Use a fresh state directory for
an independent run. These software checks do not establish external UDP reachability
or compatibility with an unspecified sender OS and OBS version.

The daemon holds an operating-system lock on its state directory. Use a separate
state directory for verification rather than starting a second coordinator against
an active project.
