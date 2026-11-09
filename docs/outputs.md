# Recording and network outputs

## Implemented ownership

Each configured output has a dedicated owner thread, GStreamer consumer pipeline, bounded queues,
desired/observed state, media-progress timestamp, and independent retry/finalization lifecycle.
Program recording consumes the typed, engine-run-namespaced
`live-<engine-run>-program-{video,audio}-v1` taps. Clean recording consumes the paired
pre-timing/pre-Program `live-<engine-run>-clean-{video,audio}-v1-<track-id>` taps. A restarted
engine therefore cannot cross-connect an old output consumer to a new graph.

Recording owners encode H.264/AAC into bounded Matroska segments. Session identity/clock metadata is atomically persisted before capture. Actual fragment-closed messages drive atomic catalog publication while recording; stop separately drains/finalizes the branch. Checksums, decode evidence, captured provenance and mappings are exposed through the coordinator. An empty or interrupted raw file is retained as partial evidence, never invented as a successful recording.

Network owners encode the same native Program taps to FLV and pipe them to an independently
supervised FFmpeg RTMPS publisher. Publisher progress is parsed from FFmpeg's machine-readable
`total_size` progress stream, so `lastMediaAtNs` reflects the network publisher rather than merely
the upstream encoder. A failed publisher is detached and retried without stopping recording or
another destination. Manual stop cancels retries. Destination URLs are stored in the
server-side SQLite `output_destinations` table through the authenticated
`POST /api/v1/output-destinations` endpoint; project configuration and snapshots contain only the
destination ID. FFmpeg runs with quiet logs at the secret-bearing boundary and errors exposed to
clients are sanitized.

Duplicate starts are idempotent and keep one owner/session. On graceful daemon shutdown, the
coordinator sends an explicit shutdown request, recorders drain/finalize, publisher children stop,
and every owner thread is joined. On restart, raw files are discovered even with missing, corrupt or stale catalogs; recovered/partial state and interruption gaps remain visible. Every
restored output has desired and actual state `stopped`; no public publisher is resumed merely
because its configuration exists.
