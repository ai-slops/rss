# Live Production

Headless Ubuntu live production operated from a browser.

Implementation baseline: [handoff](docs/implementation-handoff.md).

## Development

This is a standalone product, not a DIM feature. Development is in progress; the [evidence ledger](docs/acceptance.md) distinguishes measured software paths from remaining gates. Do not use an unchecked gate as authorization for a public broadcast.

On this assigned Alpine/musl workspace, run:

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

```sh
./scripts/acceptance.sh --quick  # strict Rust/UI checks and tests
./scripts/acceptance.sh          # also actual media, network, browser and extension gates
```

Full acceptance fails if a required gate is absent. Reports and generated fixtures stay under `var/`; the latest command report is `var/acceptance/latest.txt`. [Stage A evidence](docs/stage-a.md) records the exact measured codec profile and limits. A software fixture is not proof of hardware 1080p60/4K capacity, OBS track separation, target-laptop ICE, platform semantics or editor interoperability.

## Dependencies and Ubuntu deployment

The measured development toolchain is Rust/Cargo 1.91.1, GStreamer 1.26.9, FFmpeg 8.0.1, GCC 15.2.0, official Rust GStreamer bindings and locked React/TypeScript dependencies. `scripts/bootstrap.sh` pins native packages and installs them only under `.tools/`; `scripts/env.sh` activates them. See [the local-musl ADR](docs/adr-001-local-musl-toolchain.md). Rust and npm transitive dependencies are locked in `Cargo.lock` and `web/package-lock.json`.

On Ubuntu, have the deployment administrator supply Rust/Cargo, a C compiler, pkg-config, SQLite build prerequisites, FFmpeg, GStreamer development libraries and the base/good/bad/ugly/libav plugin sets. Required capabilities include compositor, audiomixer, H.264 CPU encoding, AAC, MPEG-TS, Matroska/splitmux, SRT and RTMPS; WebRTC additionally needs the Rust rswebrtc plugin and working ICE dependencies. Use native Ubuntu packages/libraries, not the Alpine sysroot. Build locked dependencies and rerun acceptance on the actual target before relying on performance. Hardware encoding is not assumed.

## OBS publisher profile

Provision a named authenticated input first. OBS may publish an ordinary network stream; device enrollment, Tailscale, a desktop companion and platform account credentials are not required. The baseline transport profile is SRT carrying H.264 video and AAC audio in MPEG-TS, with a bounded transport latency configured separately from manual sync correction and Program holdback. Use the source's scoped ingest credential, never an operator or platform credential.

An incoming mixed stereo track remains mixed stereo; do not advertise isolated stems unless the actual sender/plugin exposes and the receiver observes them. Real OBS/plugin track separation has not been tested in this environment. Unknown and duplicate publishers must not replace an active source. Consult the final input evidence before relying on the provisioning path.

## Production semantics

- Scene edits are Draft changes. Selecting/cueing never publishes; Take uses the exact committed scene revision and current engine run/Program revision. Preview stays selected after Take. Cue previous Program changes Preview only.
- Audio routes, gain/mute and source timing are Live/shared, independent of scene visibility. New audio is unrouted until explicitly applied. A break scene does not mute microphones.
- Listen, monitor volume/mute and panel selection are This browser controls. Monitoring starts muted. Media-disconnected and control-disconnected are different states.
- Recording and each network destination have independent desired/actual states. Start sending may immediately create a public broadcast. Configuration reload and daemon restart are not authorization to restart public output.
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

Do not automatically merge/deploy the work branch. Remaining target-system gates and final evidence belong in [the acceptance ledger](docs/acceptance.md).
