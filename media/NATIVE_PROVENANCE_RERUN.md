# Ubuntu clean-video provenance rerun

Run this check in the existing native verifier checkout after integrating the
follow-up branch. Do not boot a new guest or replace the retained verifier.

## Exact command

```sh
LIVE_PRODUCTION_NATIVE=1 CARGO_BUILD_JOBS=2 \
./target/debug/live-production-supervisor --timeout-ms 180000 --grace-ms 10000 -- \
cargo test --locked -p live-production-media \
  --test intervideo_provenance_identity -- --nocapture

LIVE_PRODUCTION_NATIVE=1 CARGO_BUILD_JOBS=2 \
LIVE_PRODUCTION_PROVENANCE_EVIDENCE_DIR="$PWD/var/ubuntu-native/clean-provenance-rerun" \
GST_DEBUG='referencetimestampmeta:7,intervideosrc:7' \
./target/debug/live-production-supervisor --timeout-ms 180000 --grace-ms 10000 -- \
cargo test --locked -p live-production \
  --test editor_clean_provenance \
  clean_recording_reconnect_provenance_survives_catalog_reload \
  -- --nocapture
```

The unmodified assertions must pass. In particular, the persisted mixed clean
fragment must contain both video and audio track intervals for continuity
generations 1 and 2, and the before-reload and after-reload provenance arrays
and gaps must remain exactly equal.

## Private artifacts to return

Return these files in the verifier evidence checkout, not merely a summary:

1. Complete command output as `clean-provenance-rerun.log`, including the
   printed `persisted clean fragment` JSON.
2. `var/ubuntu-native/clean-provenance-rerun/session.json`, copied after output
   finalization and before process teardown.
3. Every `segment-*.mkv` copied from that finalized session, plus SHA-256 for
   each. Copies happen immediately after stop, before provenance-dependent
   assertions or selection, so missing video intervals do not discard media.
4. `ffprobe -v error -show_streams -show_format -of json segment-00000.mkv`
   output and `ffmpeg -v error -xerror -i segment-00000.mkv -f null -` exit
   status.
5. GStreamer debug output from the command above, retained privately if it
   contains environment paths. Do not publish SRT credentials or operator
   secrets.

Report the exact GStreamer, FFmpeg, Rust, and application commit versions. If
the test still fails, retain the full `session.json` and fragment rather than
inferring provenance from the current source connection or rewriting catalog
metadata.

The identity test prints the full retained producer and transported
`GstMemory` layouts and `same_layout=true|false`. This follow-up deliberately
does not restore missing video metadata: local GStreamer 1.26.9 reported
different sysmem objects with no parent relationship. An exact-identity relay
must remain withheld unless the Ubuntu 1.24.2 artifact proves the entire
ordered memory layout is shared while metadata alone is absent.
The diagnostic waits for the producer sink to finish rendering, constrains both
ends to I420/16x16/30 fps, and confirms white producer pixels rather than initial
timeout-black. Its explicit metadata stripping is fault injection, not evidence
that the real target path loses metadata at this exact boundary.

## Pass boundary

This rerun proves the existing target behavior and supplies the missing
identity evidence. It does not pass the original issue unless real captured
video intervals appear in active session metadata, persist into the finalized
fragment catalog, survive coordinator reload, and the MKV fully decodes. Local
GStreamer 1.26.9 results are not a substitute.
