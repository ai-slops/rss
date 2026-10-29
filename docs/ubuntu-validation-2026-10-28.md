# Delegated Ubuntu verification receipt, 2026-10-28

## Outcome and provenance

**Native locked compilation passes. Native acceptance does not pass.** A real
Ubuntu KVM guest and software OBS startup are verified; OBS sender/plugin
interoperability is not. This is a separate verification track, not an OBS
product implementation or deployment.

- Workspace: `live-ubuntu-validation`, UUID
  `a73875ff-dba9-4785-83c1-5ab32e6d6258`, host `remote-main-4`.
- Online discovery at `2026-10-28T08:55:20.117Z` reported `container-kvm`,
  4 CPU / 12 GiB / 8192 PIDs, and available direct KVM access.
- Initial actual HEAD and fetched `origin/main`:
  `77389b4fc6071c48f918f0368b11d14d99797f51`. Initial index/worktree clean;
  baseline ancestry verified before creating `work/ubuntu-validation`.
- Guest clean clone, from a local Git bundle:
  `08a5626711f98877553ba9080a2ccdbc821fc597`. This is the baseline plus
  verification tooling only; no server/media/UI/plugin source edits.
- Tool commits: `3d2d031` (explicit native environment), `afa7291` (owned VM),
  `08a5626` (clean-source native verification).
- Managed remote heads were inspected before and after execution. Neither
  inspection advertised a primary-agent OBS harness branch. No remote workspace
  or primary session was modified. Incorporate its published harness when the
  coordinator supplies the exact branch/commit; do not replace its implementation.
- No push, merge, deployment, Studio operation, or public broadcast was performed.

## Actual guest and dependency versions

| Component | Observed |
|---|---|
| Ubuntu | 24.04.5 LTS, `ID=ubuntu`, `VERSION_ID=24.04` |
| Guest kernel | `6.8.0-142-generic` |
| Virtualization | `systemd-detect-virt`: `kvm` |
| QEMU | 10.1.5, q35, explicit `accel=kvm`, `-cpu host` |
| Guest budget | 4 vCPU, 8192 MiB configured, 7941 MiB guest-visible, no swap |
| GCC | Ubuntu `13.3.0-6ubuntu2~24.04.1` |
| Rust | `1.91.1 (ed61e7d7e 2025-11-07)`, GNU/Linux native target |
| Cargo | `1.91.1 (ea2d97820 2025-10-10)` |
| Node / npm | `v24.18.1` / `11.16.0` |
| GStreamer / app | `1.24.2` / `1.24.2` |
| GIO | `2.80.0` |
| FFmpeg | Ubuntu `6.1.1-3ubuntu5` |
| OBS Studio | `30.0.2.1-3build1`; package `30.0.2+dfsg-3build1` |
| libobs pkg-config | `30.0.2.1` |
| Qt | `6.4.2` compiled/runtime |
| OBS software renderer | Mesa llvmpipe, LLVM 20.1.2, OpenGL 4.5, Mesa 25.2.8 |

There was **no selected DIM APT mirror**. The guest used its default public
`archive.ubuntu.com` and `security.ubuntu.com` repositories; host network,
firewall, mirror configuration, and global services were not changed. APT
versions are inspected, not snapshot-pinned; the complete installed manifest is
retained as `packages.tsv`. Rust and Node versions are explicit; the Node archive
was verified against the public version's SHA-256 manifest. The downloaded
rustup bootstrap script's observed SHA-256 was
`9502b514362a21096210d498f143b8cdacda365d3dfcf45e8f5541e089ccafc0`;
the bootstrap script itself was not pre-pinned.

## Isolation, lifecycle, and exact invocation

```sh
bash scripts/ubuntu-vm.sh --help
bash scripts/ubuntu-verify.sh run
```

The runner makes one 48 GiB qcow2 overlay with the provided immutable backing:

```text
/home/dim/.cache/dim/artifacts/6a81c37564db9b1ee84e141922625e1d7c5b389b99bb3c572e0243607d5bb4d2
SHA-256: 6a81c37564db9b1ee84e141922625e1d7c5b389b99bb3c572e0243607d5bb4d2
Original image bytes: 625612288
```

The base hash matched both before boot and after shutdown. No host directory,
device other than granted KVM, Docker socket, or operator credential was mounted
into the guest. Cloud-init, client/host SSH keys, disk and logs live under private
`var/ubuntu-validation/`. SSH uses userspace NAT with only
`127.0.0.1:22428 -> guest:22`; generated Ed25519 host pin, strict checking,
disabled global known-hosts trust, no password authentication. The source was
copied as a Git bundle, not a host filesystem share.

The foreground runner uses a 5400-second process-group deadline, an exclusive
lock, and exact-child cleanup: bounded guest poweroff, TERM, KILL, and wait. It
refuses an existing overlay, so rerunning does not silently boot another VM or
overwrite evidence. Native tests and supplied media runners use the repository
supervisor. OBS startup uses its own bounded `timeout`/Xvfb process group.

- QEMU PID: `2493`; started `2026-10-28T09:09:35Z`.
- Native workload: `09:09:55Z` through `09:18:46Z`, exit **1**.
- VM stopped: `2026-10-28T09:18:52Z`, runner exit **1**.
- Exact PID `/proc/2493` was absent after shutdown; QEMU returned and was reaped.
- Runtime now: **stopped**, SSH forward closed with QEMU. The private overlay,
  seed, keys and receipts remain retained, not securely erased. No second boot.

Pre-boot Oracle review found ownership, pinning and gate-accounting defects;
these were corrected before the committed boot. Shell syntax checks, diff
whitespace checks, help/bad-input execution, native environment activation and
inherited-sysroot rejection passed. Bash LSP was unavailable; no installation
was requested. No LSP-clean result is claimed.

## Commands and results

The guest runner installed native Ubuntu build tools, GStreamer development and
base/good/bad/ugly/libav/nice packages, FFmpeg, OBS/libobs development files,
CMake, Xvfb and Mesa utilities. The exact package list and install command are
in `scripts/ubuntu-native-check.sh`, with the complete package versions retained.
It used `LIVE_PRODUCTION_NATIVE=1 CARGO_BUILD_JOBS=2` and **no Alpine sysroot**.

| Actual command / check | Exit | Observation |
|---|---:|---|
| `npm --prefix web ci --no-fund --no-audit` | 0 | Locked dependencies installed |
| `cargo build --locked --workspace` | 0 | Clean GNU/Linux native build |
| `cargo fmt --all -- --check` | 0 | Formatting passed |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | 0 | Strict lint passed |
| Supervisor, 900000 ms: `cargo test --locked --workspace` | 101 | 24 library + 5 binary unit tests passed; first integration test failed; later targets not reached |
| `bash scripts/lifecycle-acceptance.sh` | 1 | 7 process-tree cases passed; native readiness case failed |
| `bash scripts/media-stage-a.sh` | 0 | Finalized and decoded 3 clean + 3 Program MKVs and calibration MKV; measured 250.000 ms |
| `bash scripts/media-network-gates.sh` | 69 | SRT file has AAC but H.264 width/height 0; strict decode failed |
| `npm --prefix web run check` | 0 | Biome check passed |
| `npm --prefix web test` | 0 | 33 tests in 9 files passed |
| `npm --prefix web run build` | 0 | TypeScript and production build passed; existing large-chunk warning |
| `npm --prefix web audit --audit-level=moderate` | 0 | Audit passed |
| Bounded `xvfb-run -a obs --multi --verbose --disable-shutdown-check` | expected timeout 124 | Startup marker observed; real OBS initialized OpenGL and loaded bundled modules |

Required-element inspection found `compositor`, `audiomixer`, `x264enc`,
`voaacenc`, `mpegtsmux`, `splitmuxsink`, `srtsrc`, `srtsink`, and `rtmpsink`.
**`webrtcsink` was missing**, independently making the overall run fail.

Stage A profile: clean H.264 640x360 + stereo AAC 48 kHz; Program H.264
1280x720 + stereo AAC 48 kHz. Its separate calibration file contains two mono
AAC streams; this is synthetic timing evidence, not OBS isolated-stem evidence.
WebRTC was not enabled for this Stage A run.

## Concrete compatibility defects for the coordinator

1. **Stock Ubuntu dependencies do not supply the required `webrtcsink`.**
   `gst-inspect-1.0 webrtcsink` fails after the native package installation.
   Supply/review a native rswebrtc installation before claiming daemon/browser
   monitoring readiness. `scripts/browser-install.sh` and
   `scripts/browser-chromium.sh` also remain Alpine-specific; browser acceptance
   was not attempted or passed here.
2. **Clean recording provenance fails on this actual native stack.**
   `cargo test --locked -p live-production --test editor_clean_provenance`
   is the targeted rerun command. The observed test
   `clean_recording_reconnect_provenance_survives_catalog_reload` fails at
   `server/tests/editor_clean_provenance_support/mod.rs:49`: the persisted
   mixed fragment reports generation-1 and generation-2 audio provenance but
   no video-track interval. The log includes the observed recording metadata.
   Root cause is not established; do not relabel this pass or weaken the test.
3. **Native daemon lifecycle readiness is not established.** The first case in
   `scripts/native-lifecycle.test.mjs:32` reports that the native fixture exited
   before readiness (`var/lifecycle-YuOqS3` in the guest). Seven supervisor
   process-tree tests pass, which is not a pass for the 11 native-app outcomes.
   Missing rswebrtc is a candidate dependency issue, not a confirmed root cause;
   the fixture's private server log remains on the retained guest disk, not in
   the copied receipt archive.
4. **The existing SRT network fixture is not decoder-compatible here.**
   `srt-received.ts` reports 4.885333 seconds / 683756 bytes and stereo 48 kHz
   AAC, but H.264 dimensions are `0x0` and FFmpeg reports `non-existing PPS 0
   referenced`. Strict decode exits 69. The RTMP receiver was exercised, but
   the script aborts validating SRT before RTMP-file validation; neither gate
   is claimed passed. Root cause needs investigation against GStreamer 1.24.2
   and FFmpeg 6.1.1; transport arrival alone is not usable-video proof.
5. **Acceptance runners previously required an Alpine sysroot even on Ubuntu.**
   Commit `3d2d031` adds opt-in native mode in `scripts/env.sh`, leaves default
   Alpine behavior unchanged, and rejects inherited sysroot settings. Existing
   lifecycle and media runners were actually exercised through this mode.

These findings belong to the primary/coordinator handoff. No OBS plugin,
sender, media-engine rewrite, or competing product harness was added here.

## Evidence retained and exchange

Private receipts are under `var/ubuntu-validation/`; extracted native logs are
under `var/ubuntu-validation/var/ubuntu-native/`. They contain command output,
element inspection, versions, installed packages, and OBS startup output.
Actual generated media and fixture-state logs not explicitly archived remain
on the stopped guest overlay. Do not publish the disk, cloud-init seed, SSH keys,
or OBS configuration directory as public evidence.

| Artifact | SHA-256 |
|---|---|
| `receipts.tar.gz` | `c1f6a530f4e47daf11772ff7a5708eb1449672c3b53b215baba52d2c595f84cf` |
| `source.bundle` | `acdd2fca0f0cf4a21a3dc2558fcc656136e1bfb17b7e76845691aa28a019df09` |
| `guest-inventory.txt` | `2b8df23b2178b1528662cdd475c90155db210141a981b22d629c648bb527eff4` |
| `packages.tsv` | `4088d4277ca1bb7fc6dcf73c989491352b9f0a6124a97a1053fe39b0d03549fa` |
| `results.tsv` | `f3a8e713a725817f44e0cd293ab97df4bf77655caf7de8047faae001f6b6e756` |
| `rust-tests.log` | `80aa9ad6b7055a0fea04e3041bab456a20c0b3ca0f6b504af08f7ac1c9348e0e` |
| `lifecycle.log` | `073cb34f868f51aa601b6e95127e9c5ed096104e7b9bcddaf5f8703940c37336` |
| `media-stage-a.log` | `198340f521a678257369ff3fd648235bb1a7672ded69dcadc7fd4322e04c4d60` |
| `media-network.log` | `46a794695aafb32ed689c30da830c5163c3cdde5707a5f4a1e6f9567df60a4a1` |
| `obs-startup.log` | `1b106618dcea3c4f3face3005175332fd167eaed897b6a62fc8bfbf0395e0008` |

## Readiness and remaining gates

The environment has demonstrated real KVM boot, pinned private SSH, native
locked compilation and software OBS initialization. It is **not a passing
Ubuntu production candidate** until the dependency and runtime failures above
are resolved and their affected gates rerun. No unattended repeat boot is
scheduled. Keep this one guest stopped until the coordinator supplies the
published harness and coordinates any further runtime authorization.

Still unverified: primary plugin build/load and OBS-to-authenticated-SRT media,
bad-auth/duplicate/reconnect with that OBS sender, actual OBS track fidelity,
Ubuntu end-to-end clean+Program/editor package, full recovery/drift/soak,
Ubuntu browser media, target laptop OS/browser/editor, physical capture and
hardware encoding, external-network ICE, intended editor application import,
and platform-account/broadcast semantics. No platform output is authorized by
these receipts. OBS startup reported no hardware VAAPI and disabled XComposite
without an EWMH window manager; those are expected software-fixture limits, not
physical-device validation.
