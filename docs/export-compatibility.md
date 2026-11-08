# Portable editor export compatibility

`scripts/target/export-compatibility.mjs` verifies an existing editor-session export and builds a neutral portable package. It does not generate OTIO, FCPXML, Premiere XML, Resolve projects, or any other editor-specific schema.

The target editor is unspecified. A successful run proves structural consistency, media integrity, observed stream compatibility, and complete FFmpeg decoding. It does not prove import into an intended application.

## Entry point

Run from the repository root:

```sh
node scripts/target/export-compatibility.mjs \
  --input /private/path/editor-package.json \
  --output var/target/export-package-unique \
  --supervisor /path/to/live-production-supervisor \
  --ffprobe /usr/bin/ffprobe \
  --ffmpeg /usr/bin/ffmpeg
```

The output directory must not already exist. If `--output` is omitted, a unique private directory is created under `var/target/`. `LIVE_PRODUCTION_SUPERVISOR`, `FFPROBE`, and `FFMPEG` may provide the corresponding executable paths.

The input may be the editor package itself or the saved public command response containing it under `pluginResult`.

Every FFprobe and FFmpeg child is launched through `live-production-supervisor` with a bounded timeout. The harness does not launch the production daemon or contact platform services.

Exit status is zero only when structural checks and full media decoding pass. Application import remains `notRun` and does not affect the exit status.

## Checks

Structural verification covers:

- Package schema version and stable session/project/engine-run identities.
- Unique recording and output identities.
- Decimal-string nanosecond fields, including values above JavaScript's safe integer range.
- Timebase mapping and matching engine-run identity when available.
- Per-file media bounds, mapping state, uncertainty, provenance intervals, and gaps.
- Stable source and track IDs and mixed continuity generations without collapsing them to one generation.
- Explicit missing Program reference and missing clock mapping issues.
- Recovery and validation states without promoting partial or unmapped media.
- Program recordings not claiming clean-source provenance.

Media verification covers:

- Source file existence and regular-file type.
- Declared byte size.
- SHA-256 equality before copying.
- FFprobe stream discovery with at least one video and one audio stream.
- Full `ffmpeg -xerror` decode of every eligible recording.
- SHA-256 and byte-size equality after copying.

Files marked partial or not decode-validated are reported as decoder failures and are not copied as valid clips. Missing files and checksum mismatches are structural failures. Legacy unknown provenance or timebase remains explicitly unknown.

## Artifact layout

```text
export-package-<unique>/
  manifest.json
  metadata/
    editor-package.json
    compatibility-report.json
  media/
    0000-<recording-id>.mkv
    ...
```

The copied editor package uses relative `media/...` paths. The manifest contains recording IDs, relative paths, hashes, byte counts, recovery state, and mapping state. Original absolute paths are not written into the package or report.

Sensitive fields whose names indicate passwords, passphrases, credentials, stream IDs, tokens, cookies, authorization, or secrets are replaced with `[REDACTED]`. The input JSON and original media are read-only and remain unchanged.

## Compatibility statuses

The report separates three outcomes:

- `structural`: package, lineage, hash, size, and mapping consistency.
- `decoder`: negotiated streams and complete FFmpeg decoding.
- `applicationImport`: always `notRun` with reason `targetEditorUnspecified` until a target application is chosen and tested.

`reconstruction` remains the server's declared capability, such as `approximate` or `program-only`. The harness does not upgrade it. Missing Program references, unavailable clock mappings, mixed generations, partial recovery, and gaps remain visible limitations.
