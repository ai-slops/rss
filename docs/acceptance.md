# Release evidence ledger

This ledger separates observed results from required gates. An unchecked item is not a pass.

## Environment and recovery

- Baseline repository: `12903d8`.
- Work branch: `work/initial-production`.
- Initial probes: Node `v24.18.1`, npm `11.11.0`; Rust, Cargo, GStreamer, FFmpeg, pkg-config, native C compiler and system package tools were not on PATH.
- Initial push: local checkpoint committed; push timed out after 120 seconds. A bounded direct origin connection probe timed out after 10 seconds.
- Initial `dim-expose list`: `dim-workspace: fetch failed`. No reachable interface URL has yet been established.
- Context7 documentation retrieval was unavailable due to service monthly quota. Official documentation remains the reference fallback.

## Required controlled gates

| Gate | Evidence needed | State |
|---|---|---|
| Two-source video and two independent audio tracks | Real decoded/composed media and negotiated formats | Pending |
| +250 ms correction | Combined recording measurement, tolerance one frame plus measured audio block | Pending |
| Feasible/infeasible signed correction | Actual retained history and rejected infeasible transaction | Pending |
| Bounded live timing transition | Recorded discontinuity and progress/memory observations | Pending |
| Drift | Long-run independent-clock observation and correction/reporting | Pending |
| Authoritative Preview and Program | Actual server-rendered browser media with separate control/media state | Pending |
| Exact Take and draft isolation | Browser workflow plus recorded revision/timestamp evidence | Pending |
| Clean and Program recordings | Independent segment finalization, decode and checksums | Pending |
| Local RTMPS plus second receiver | Actual encoded delivery; disconnect/stall isolation | Pending |
| Authenticated ingest/reconnect | Rejected unknown/duplicate publisher, stable IDs and continuity gaps | Pending |
| Storage faults | Affected recorder fails truthfully; unrelated media progresses | Pending |
| Restart/browser reconnect | Public outputs remain stopped; no replayed live commands | Pending |
| Concurrency | Stale writes and undo rejected; local work recoverable | Pending |
| Extension proof | Editor package reconciles source/reference/history; example uses public commands | Pending |
| Independent reviews | Goal-based browser and engineering findings addressed | Pending |

## External gates not supplied

Real OBS/plugin source separation, a target laptop's remote ICE path, platform account auto-start semantics/private test broadcast, hardware encoder performance and intended editor import are unverified until those exact systems are exercised. Controlled software fixtures cannot establish these integrations.
