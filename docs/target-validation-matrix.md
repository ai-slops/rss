# Native OBS validation

Build and run the current source on native Ubuntu using `scripts/ubuntu-obs-check.sh`.
For an isolated VM, run `bash scripts/ubuntu-verify.sh run`.
Logs and private fixtures stay under `var/`; source documentation does not certify a build.

## Operator entrypoints

```sh
# Native Ubuntu capability and ABI inspection. Do not source scripts/env.sh.
scripts/target/preflight.sh \
  --plugin-dir /private/stage \
  --report /private/preflight-inspection.json

# Full supervised product rehearsal; creates a new root and runs preflight itself.
scripts/target/rehearsal.sh \
  --plugin-dir /private/stage \
  --evidence-dir /private/evidence

# Contract-only local check. Launches no server, OBS, Xvfb or publisher.
scripts/target/rehearsal.sh \
  --plugin-dir /private/stage \
  --evidence-dir /private/evidence-dry \
  --dry-run
```

Optional arguments are `--server-binary`, `--supervisor-binary`, `--obs-binary`,
`--xvfb-binary`, `--web-dist`, `--publisher-host` and `--deadline-seconds`.
The evidence root must not exist before a run; its parent directory must exist.
A standalone preflight report must therefore not pre-create that evidence root.

The full runner itself executes beneath the repository-built native supervisor. In addition, every
server, OBS and Xvfb launch receives its own bounded invocation of that same supervisor; the Node
harness never directly owns a native application process. Crash tests signal the supervised native
child PID while the wrapper remains the subreaper and cleanup authority. No `nohup`, `setsid`,
detached preview, service registration or endless process is used. All outcomes run cleanup; a
stopped child receives `SIGCONT` before supervisor termination.

## Native preflight

Preflight requires and records actual versions for OBS/libobs, GStreamer, FFmpeg, Rust/Cargo,
Node/npm, the C compiler and pkg-config. Repository development pins are reported separately from
target observations; they are not misreported as installed Ubuntu versions.

Required properties:

- `/etc/os-release` identifies Ubuntu.
- `ldd --version` identifies GNU libc, not musl.
- No `.tools/sysroot`, Alpine or musl library/plugin/sysroot environment leaks into the run.
- Native server, supervisor, OBS and staged plugin ELF dependencies resolve without Alpine paths.
- OBS links libobs.
- GStreamer provides compositor, audiomixer, splitmuxsink, x264enc, voaacenc, SRT, RTMPS,
  Matroska, MPEG-TS, H.264/AAC parse/decode, webrtcsink and nicesrc.
- FFmpeg exposes H.264/AAC encode and decode capabilities.

The plugin stage must contain `obs-live-production.so` and the executable
`obs-live-production-srt` under `bin/64bit`, plus module data
under `data/obs-plugins`. The rehearsal supplies those paths through OBS plugin environment
variables; it does not copy into a user profile or system directory.

## Publisher contract v1

Bundles are private JSON generated only after authenticated source creation, a fresh snapshot and
the authenticated publishing-endpoint response. No ID is guessed.

```json
{
  "schemaVersion": 1,
  "target": {
    "sourceId": "observed",
    "videoTrackId": "observed",
    "audioTrackId": "observed",
    "host": "127.0.0.1",
    "port": 12345,
    "passphrase": "private",
    "streamId": "private",
    "bufferMs": 120
  },
  "obs": { "sourceName": "OBS Red 440", "audioMixIndex": 0 },
  "encoding": {
    "width": 1280,
    "height": 720,
    "fpsNumerator": 30,
    "fpsDenominator": 1,
    "audioSampleRate": 48000,
    "videoBitrateKbps": 2500
  }
}
```

Repository-local contract checks, which make no native OBS claim:

```sh
node --test scripts/target/harness/*.test.mjs
scripts/target/rehearsal.sh \
  --plugin-dir var/private-plugin-stage \
  --evidence-dir var/private-rehearsal-dry \
  --dry-run
```

`sourceUuid` is emitted only when explicitly supplied by a supported plugin implementation.
Audio mix indexes are restricted to `0..5`.

OBS receives only publisher-bundle paths through the private fixture file:

```json
{
  "schemaVersion": 1,
  "localFixtureOnly": true,
  "senders": ["/private/red.publisher.json", "/private/blue.publisher.json"],
  "reportFile": "/private/obs-fixture-report.json",
  "controlFile": "/private/obs-fixture-control.json",
  "deadlineSeconds": 300
}
```

The harness passes that file only through
`LIVE_PRODUCTION_OBS_FIXTURE_FILE=/private/obs-fixture.json`. The plugin must treat the fixture as
local test input and must not read operator cookies/passwords or server project storage.

The private control file uses this command shape:

```json
{
  "schemaVersion": 1,
  "commands": [
    {
      "kind": "configureNativeFixtureSource",
      "senderBundle": "/private/red.publisher.json",
      "sourceId": "observed",
      "sourceName": "OBS Red 440",
      "color": "red",
      "frameCounter": true,
      "toneHz": 440,
      "audioMixIndex": 0
    },
    { "kind": "startBadAuthSender", "senderBundle": "/private/bad-auth.publisher.json", "expected": "rejected" },
    { "kind": "startDuplicateSender", "senderBundle": "/private/red.publisher.json", "expected": "rejected" }
  ]
}
```

The plugin report consumed by the harness is versioned and must include at least:

```json
{
  "schemaVersion": 1,
  "moduleLoaded": true,
  "obsVersion": "actual",
  "module": "obs-live-production",
  "state": "ready",
  "senders": [
    {
      "sourceId": "observed",
      "state": "sending",
      "audioMixIndex": 0,
      "visual": { "color": "red", "frameCounter": true },
      "audio": { "toneHz": 440 }
    }
  ],
  "negativeCases": [
    { "kind": "badAuth", "result": "rejected" },
    { "kind": "duplicateSender", "result": "rejected" }
  ]
}
```

The control file declares two native OBS media sources: red plus frame counter plus 440 Hz on mix
0, and blue plus frame counter plus 880 Hz on mix 1. It also requests an invalid-auth sender and a
duplicate sender. The OBS report must prove module load, real sender state, native source
declarations, explicit mix routing and rejection of both negative cases.

## Workflow matrix

| Gate | Harness action | Required evidence |
|---|---|---|
| Auth | Private operator password file; cookie held only in memory | No password/cookie in report or URL |
| Sources | Create two SRT sources on ephemeral loopback UDP ports | Snapshot IDs equal bundle IDs |
| OBS | Start real OBS under Xvfb with staged plugin | Process report proves module loaded |
| Content | Native red/blue frame-counter sources, 440/880 Hz audio | Decoded clean pixels change and tones dominate |
| Admission | Invalid passphrase and duplicate sender | Both rejected; active sources remain connected |
| Draft/Live | Add both sources, save side-by-side Draft, explicit audio routes and timing | Program unchanged before Take; routes survive Take |
| Recording | Program plus both clean-source recorders | H.264/AAC, 48 kHz, decode and checksum pass |
| Separation | Compare each clean source independently | Report `per-source clean`; never call mixed input stems |
| Recovery | Stop on fourth fragment creation, at least two closed, hash, SIGKILL, restart | Raw hashes unchanged; exact partial tail visible; outputs stopped |
| Export | Editor session export after recovery | Media inventory includes recovered files and interruption gaps |
| Monitoring | Separate preview owner validation | Not claimed by this harness; preview owner supplies evidence |
| Public output | None | Explicitly not tested |
| External editor import | Unspecified target editor | Pending; does not stop other gates |

Every recording segment, including short one-second splitmux segments, must contain exactly one
H.264 video stream and one stereo AAC stream at 48 kHz. Clean recordings must be `1280x720`;
Program must match the project canvas. Content analysis is grouped per output: frames are sampled
at 2 fps across all segments so a short segment is not rejected merely for yielding one 1-fps
sample. Each clean output must prove its declared color, changing counter and dominant tone.
Program must prove red-left/blue-right composition and both routed 440/880 Hz tones.

## Ports and private artifacts

HTTP uses an ephemeral loopback TCP port. Each SRT source uses a separately allocated ephemeral
loopback UDP port. Xvfb chooses an unused private display from `:90` through `:120`.

The evidence directory is mode `0700`; authentication, bundles, fixture control/report and server
state are mode `0600` or private subdirectories. The public `rehearsal-report.json` contains IDs,
capability versions, hashes and sanitized declarations only. It excludes passwords, cookies,
passphrases, stream IDs, secret URLs and private absolute paths.

Expected private artifacts:

- `private/preflight.json`
- `private/operator-auth.json`
- `private/red.publisher.json`, `private/blue.publisher.json`, `private/bad-auth.publisher.json`
- `private/obs-fixture.json`, `private/obs-fixture-control.json`, `private/obs-fixture-report.json`
- isolated state/recordings and native process logs
- `rehearsal-report.json` (sanitized summary)
