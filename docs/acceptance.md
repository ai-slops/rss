# Release evidence ledger

This ledger separates observed results from required gates. An unchecked item is not a pass.

## Environment and recovery

- Baseline repository: `12903d8`.
- Work branch: `work/initial-production`.
- Initial probes: Node `v24.18.1`, npm `11.11.0`; Rust, Cargo, GStreamer, FFmpeg, pkg-config, native C compiler and system package tools were not on PATH.
- Initial push: local checkpoint committed; push timed out after 120 seconds. A bounded direct origin connection probe timed out after 10 seconds.
- The single supervised `studio` service is running on loopback `3100`. The authenticated shared-HTTPS interface is `https://live-production--studio-6d4ac2cc57021bab.mj-y.com/`. The final read-only browser check decoded both `960x540` monitors with no page errors and confirmed every output remained stopped. This same-container check does not establish a target laptop's remote ICE path.
- Approved code/design/ADR work is preserved in 47 ordinary local commits ending at `ef2e70db8c37fbd3f4eebfb20c4fba2e5331d5ed`. Recovery history excludes runtime state, credentials, caches and media. The handoff bundle is `var/checkpoints/initial-production-complete.bundle`; an unreachable origin is not counted as a successful push.
- Context7 documentation retrieval was unavailable due to service monthly quota. Official documentation remains the reference fallback.

## Latest controlled run

The current implementation and **all requested feasible internal gates pass** at code checkpoint `dea49063eb99d54f927a500d3d52216a0aec2acf`. Complete required acceptance started **2026-10-27 22:48:49 UTC** in detached `var/internal-validation.TRI6kS` and exited zero. Its own `target/`, bundle, state and fixtures kept the running studio unchanged. Exact report: `var/internal-validation.TRI6kS/var/acceptance/latest.txt`. See [internal gate implementation and measured evidence](internal-gates-2026-10-27.md) for SIGKILL raw hashes, simultaneous publisher/failed-handshake proof, three-minute timing/drift metrics, clean provenance and reviewer approval. Frontend tests now total **33**. No merge/deploy or public output was performed; studio remains the prior running revision.

### Earlier checkpoint evidence

`./scripts/acceptance.sh`, started **2026-10-27 14:34:21 UTC**, exited successfully against committed code checkpoint `ef2e70db8c37fbd3f4eebfb20c4fba2e5331d5ed`. Only these evidence-ledger updates followed the tested code. The command report is `var/acceptance/latest.txt`.

- All workspace Rust tests, formatting, strict all-target Clippy, and the 250-code-line source-size gate passed.
- Frontend Biome, all 26 tests, strict TypeScript and production build passed; npm audit reported zero vulnerabilities.
- Stage A finalized and decoded clean/Program segments and measured exactly `250.000 ms` between the calibration tracks. Artifacts: `var/media-stage-a.7r3dHH/`.
- Loopback SRT and RTMP decode gates passed. Artifacts: `var/media-network.42wFOu/`.
- Recording, two controlled RTMPS receivers, stall isolation, graceful shutdown/reopen, and rendered output controls passed. Artifacts: `var/output-acceptance/`.
- Real Chromium decoded both `960x540` server monitors, displayed the selected synthetic source before Take, took the exact revision, and received selected-source audio on its own connected peer. Private Listen did not change audio routes, and closing the page did not change Program. Routing/unrouting, selection-only clicks, persisted selection, sequential numeric crop/opacity, and immediate Escape cancellation passed through the UI. No autoplay/ICE policy bypass was enabled. Artifacts: `var/browser-acceptance.CfmxuQ/`.
- The editor public-command tests exported real Program recordings with per-file mappings. The native recording test located green/red transitions in different decoded fragments and reconciled their timestamps to the authoritative Program Take timestamps.
- Eleven native lifecycle outcomes restored the process baseline from zero to zero; a post-suite process scan also found zero native daemons.
- The retained UI reviewer passed fresh coverage of all eight operation/library states at `1280`, `768`, and `375` widths, plus six focused Inspector/scene-action captures. Actual CJK glyphs and panel-local bounds were inspected, not inferred from DOM dimensions. Evidence: `var/visual-review/fixed-review-20261027-04/` (30 PNGs). The earlier short-height and `640x450` CSS-viewport/DPR2 checks passed; that method is not claimed as native browser zoom.
- The final shared-HTTPS read-only check is `var/live-readonly-evidence.json`, with `var/live-interface.png`. After isolated-test cleanup, exactly one intended studio daemon was started through `dim-workspace service resume`; no disposable test service remains registered.

The browser evidence records the initial unauthenticated `401` response. The build retains a bundle-size warning above 500 kB, and the intentionally stopped transport receiver reports an I/O error during shutdown. These were not hidden or treated as evidence of unsupported external integrations. Rust/TypeScript LSP diagnostics could not run because the servers are not installed and installation was previously declined; compiler, Clippy and TypeScript checks provide the available static validation.

## Required controlled gates

| Gate | Evidence needed | State |
|---|---|---|
| Two-source video and two independent audio tracks | Real decoded/composed media and negotiated formats | Pass for synthetic Stage A profile; external sender separation unverified |
| +250 ms correction | Combined recording measurement, tolerance one frame plus measured audio block | Pass: `250.000 ms` combined AAC fixture |
| Feasible/infeasible signed correction | Actual retained history and rejected infeasible transaction | Pass for controlled synthetic and authenticated SRT tests |
| Bounded live timing transition | Recorded discontinuity and progress/memory observations | Pass: three-minute native fixture, three signed transactions, real recorded pixels/PCM, bounded queues/RSS |
| Drift | Long-run independent-clock observation and correction/reporting | Pass for measured three-minute profile: positive/negative receiver-ingress estimates, fit statistics, bounded history and reconnect reset |
| Authoritative Preview and Program | Actual server-rendered browser media with separate control/media state | Pass locally: decoded video and selected-source audio; remote ICE unverified |
| Exact Take and draft isolation | Browser workflow plus recorded revision/timestamp evidence | Pass for controlled browser, pinned-history, pixel/PTS and cancellation regressions |
| Clean and Program recordings | Independent segment finalization, decode and checksums | Pass: `docs/output-evidence.md` |
| Local RTMPS plus second receiver | Actual encoded delivery; disconnect/stall isolation | Pass: `docs/output-evidence.md` |
| Authenticated ingest/reconnect | Rejected unknown/duplicate publisher, stable IDs and continuity gaps | Pass: barrier-started native publishers, peer leases, wrong-secret/correct-streamid recovery, stable IDs and generation-2 reconnect |
| Storage faults | Affected recorder fails truthfully; unrelated media progresses | Pass: `docs/output-evidence.md` |
| Restart/browser reconnect | Public outputs remain stopped; no replayed live commands | Pass: actual SIGKILL/reopen discovers two closed files and partial tail, preserves all raw hashes/gaps/export inventory; restored outputs stopped |
| Concurrency | Stale writes and undo rejected; local work recoverable | Pass: stale server writes, two-browser conflict recovery and two-view stale undo preserves the other accepted edit |
| Extension proof | Editor package reconciles source/reference/history; example uses public commands | Pass: captured clean source/video/audio generation intervals and gaps, proportional per-file mapping, precise JSON and reload; actual editor import remains external |
| Independent reviews | Goal-based browser and engineering findings addressed | Original engineering reviewer APPROVE after correcting blockers; isolated real-browser recovery/drift/provenance proof passes. Universal audit/performance certification is not claimed |

## Correctness repairs covered by this run

- New source audio starts physically muted, approved gain survives reconnect, omitted routes are muted, and unpublished SRT sources retain unknown/null meters rather than reporting inter-pipeline fallback silence as measured input.
- Failed frame-boundary timing commands are cancelled under the same lock as commit. Resuming a paused Program cannot apply a command that already reported failure; a committed result wins over a delayed notification.
- Dynamic compositors retain a hidden live clock input, preventing the padless-to-first-input timestamp reset observed during preparation. Producer reference timestamps survive native video taps; recorder mappings account for observed bridge cadence rather than equating independent pipeline clocks.
- A real-browser negative regression proves the private-audio wait cannot pass without a source stream. The old async `waitForFunction` predicate returned too early; awaited polling now verifies the selected peer's actual packets.
- Routing captures checkbox values synchronously instead of retaining an expired React event. Canvas gestures freeze their coordinate transform, distinguish clicks from drags and cancel immediate Escape before workspace shortcuts; saves retain selection. Transport-error recovery does not erase genuine command failures. Explicit layout rows, wrapping actions and pinned local CJK fonts address the reviewed clipping and missing-glyph defects.

Controlled acceptance is not an external production certification. The internal gates above are implemented and measured, not deferred. Three-minute bounded software evidence is not a multi-day soak or universal clock/capture alignment guarantee. The intended editor's import and target Ubuntu/hardware/OBS/platform/remote-laptop behavior below remain unverified. No public broadcast was started and the running studio was not updated.

## External gates not supplied

Real OBS/plugin source separation, a target laptop's remote ICE path, platform account auto-start semantics/private test broadcast, hardware encoder performance and intended editor import are unverified until those exact systems are exercised. Controlled software fixtures cannot establish these integrations.
