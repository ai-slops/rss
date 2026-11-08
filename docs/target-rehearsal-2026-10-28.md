# Target rehearsal checkpoint: 2026-10-28

## Status and ownership

This is a reviewed checkpoint for native Ubuntu verification, not a completed
Ubuntu/OBS acceptance claim. Work is isolated on `work/target-rehearsal-20261028`,
based on `77389b4fc6071c48f918f0368b11d14d99797f51`. The existing studio was not
rebuilt, restarted, deployed, or used as the rehearsal state directory.

The retained runtime, outputs, extensions, preview, UI, and reviewer sessions
kept their original ownership. The separate `live-ubuntu-validation` workspace
owns Ubuntu provisioning and native execution. Its advertised OpenCode endpoint
returned HTTP 401 to this workspace; no credentials were guessed and no duplicate
VM or replacement verification agent was launched.

## Implemented surface

- Official libobs/OBS frontend plugin, with a dedicated source/scene video view,
  one explicit stereo audio mix, independent authenticated SRT output, and a
  native Tools dialog requiring explicit confirmation and Start/Stop.
- Publisher bundles contain only scoped ingest credentials. No operator or
  platform credentials enter OBS. Secrets are separate service fields, not
  included in the SRT URL or public report. No automatic publisher restart.
- Genuine OBS fixture source implementation produces red/blue frame counters
  and separate 440/880 Hz mix fixtures. Negative cases observe real output
  failures; declarations alone cannot pass the receiver's decode checks.
- Supervised target preflight/rehearsal, exact media-profile/content checks,
  fresh private evidence roots, real HTTP workflow driver, and abrupt recovery.
- Private native STUN/TURN/UDP-range configuration and authenticated nonsecret
  capability discovery. Browser ICE-server provisioning remains unsupported and
  is reported as such, not hidden behind a successful HTTPS control check.
- Portable export integrity/full-decode harness. Application import remains
  `notRun/targetEditorUnspecified`; mixed stereo is never called isolated stems.

## Checks actually executed here

The local environment is Alpine/musl, Rust 1.91.1, GStreamer 1.26.9, FFmpeg
8.0.1, and rswebrtc `0.14.3-RELEASE`. These are not Ubuntu observations.

1. Full isolated `bash scripts/acceptance.sh`, started
   `2026-10-28T11:18:47Z`, exited 0. Strict workspace Clippy/formatting, Rust
   tests, 33 frontend tests, TypeScript/Biome, web build, dependency audit,
   lifecycle cleanup, media/network, output independence, SIGKILL recovery,
   browser media/control, and editor extension gates passed. The existing
   182-second independent-clock soak passed; no longer Ubuntu/OBS soak is claimed.
   Evidence: `var/acceptance/latest.txt` and the gate artifact directories.
2. After the harness corrections, the final target test invocation enabled both
   real native drivers and export decoding: **27 passed, 0 failed, 0 skipped**.
   It imported the harness through a live isolated HTTP server and supervised
   synthetic GStreamer SRT senders, not OBS. Evidence:
   `var/target-rehearsal-driver.oKdBDJ/driver-result.json`: 27 clean/Program
   segments, 4 crash fragments, at least 2 confirmed closed, unchanged raw hashes,
   and 31 editor inventory entries.
3. Final standalone crash gate passed at `var/output-crash-parent-qa`: four raw
   fragments, two confirmed closed, partial/recovered classifications, preserved
   hashes, and stopped outputs after restart. The fixture now stops on fragment
   creation instead of polling late enough to capture an already-decodable tail.
4. The parent drove the current monitoring script through a real isolated
   browser at `2026-10-28T12:46:35.260Z`. Evidence:
   `var/monitor-parent-qa/monitor-report.json`. Both 960x540 videos changed;
   Program revision and shared route settings stayed unchanged. A distinct third
   audio-only peer created after Listen matched the browser audio element and
   received four packets. All three ICE peers connected over local host/host UDP.
   This is not remote-laptop or shared-ingress media evidence.
5. Strict C++17 SRT URL test compiled and executed. The frontend public header
   compiled independently after correcting its incomplete-type constructor and
   dialog namespace. Full Qt/libobs compilation and rendered Tools-dialog QA
   could not run here because those development/runtime dependencies are absent.
   LSP diagnostics were unavailable because the relevant servers were not
   installed and installation had previously been declined.
6. The retained independent reviewer approved this checkpoint **for native
   verifier handoff**, explicitly not native product readiness. The filesystem
   creation watcher can still fail closed on a missed event; it cannot manufacture
   a partial-tail pass.

The web build retains an existing greater-than-500-kB chunk warning and dependency
annotation warnings. The browser acceptance report includes an expected
unauthenticated 401. Neither was hidden by changing validation thresholds.

## Native verifier handoff

Fetch the checkpoint branch normally and record `git rev-parse HEAD`. Use native
Ubuntu dependencies, never `scripts/env.sh` or the Alpine sysroot. Report actual
OS, compiler, Qt, OBS/libobs, FFmpeg, GStreamer, and rswebrtc versions.

```sh
cargo build --locked --release --workspace
npm --prefix web ci
npm --prefix web run build
cmake -S integrations/obs-live-production \
  -B integrations/obs-live-production/build -DCMAKE_BUILD_TYPE=RelWithDebInfo
cmake --build integrations/obs-live-production/build --parallel 2
ctest --test-dir integrations/obs-live-production/build --output-on-failure
cmake --install integrations/obs-live-production/build \
  --prefix "$PWD/integrations/obs-live-production/package"

# Parent directory must exist; evidence root must not already exist.
scripts/target/rehearsal.sh \
  --plugin-dir "$PWD/integrations/obs-live-production/package" \
  --evidence-dir /private/new-rehearsal-run \
  --server-binary "$PWD/target/release/live-production" \
  --supervisor-binary "$PWD/target/release/live-production-supervisor" \
  --obs-binary /usr/bin/obs
```

The runner performs preflight automatically. A standalone preflight must write
outside the subsequently chosen fresh rehearsal root. See
`target-validation-matrix.md` for capabilities, ports, fixture contract, and
artifacts. Then run the manual native Tools-dialog procedure in
`../integrations/obs-live-production/README.md` with a supervised private fixture.

## Gates still open

- Native Ubuntu build/link, genuine OBS plugin load, real source/mix transmission,
  bad-auth/duplicate admission, clean unload, and rendered Tools-dialog validation.
- Longer native OBS soak/fault profile and actual hardware/encoder capacity.
- Actual laptop media across approved UDP forwarding or a supplied TURN service;
  HTTPS control alone does not satisfy this gate.
- Intended editor choice and actual application import.
- Platform/public broadcasting: intentionally not started and not authorized by
  this rehearsal. No merge or deployment is performed by publishing the branch.
