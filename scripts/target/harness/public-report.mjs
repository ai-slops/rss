import { publicPath } from "./private-files.mjs"

export function buildPublicReport(input) {
  return {
    schemaVersion: 1,
    status: "pass",
    nativeExecution: true,
    claims: {
      obsProcessModuleLoaded: true,
      softwareVirtualDisplayOnly: true,
      publicRtmpsTested: false,
      observedSeparation: "per-source clean recordings; no stem claim",
    },
    preflight: { status: input.preflight.status, actual: input.preflight.actual },
    sources: input.bundleRecords.map((record) => ({
      sourceId: record.bundle.target.sourceId,
      videoTrackId: record.bundle.target.videoTrackId,
      audioTrackId: record.bundle.target.audioTrackId,
      endpoint: record.endpoint,
      declaration: {
        key: record.declaration.key,
        name: record.declaration.name,
        color: record.declaration.color,
        toneHz: record.declaration.toneHz,
        audioMixIndex: record.declaration.audioMixIndex,
      },
      bundle: publicPath(record.path, input.evidenceDirectory),
    })),
    fixture: {
      obsVersion: input.fixtureReport.obsVersion,
      module: input.fixtureReport.module,
      negativeCases: input.fixtureReport.negativeCases.map((entry) => ({ kind: entry.kind, result: entry.result })),
    },
    production: {
      programRevisionBeforeDraft: input.production.programRevisionBeforeDraft,
      appliedProgramRevision: input.production.snapshot.program.programRevision,
    },
    recordings: input.recordingEvidence.map(({ output, recordings, analyzed }) => ({
      outputId: output.id,
      outputKind: output.adapter.kind,
      segmentCount: recordings.length,
      segments: analyzed.segments,
      content: analyzed.content,
    })),
    crashRecovery: publicCrashRecovery(input.crash),
    editor: {
      mediaInventoryCount: input.exported.mediaInventory.length,
      gaps: input.exported.gaps,
      issues: input.exported.issues,
    },
  }
}

export function publicCrashRecovery(crash) {
  return {
    outputId: crash.outputId,
    rawFragments: crash.rawFragments,
    confirmedClosedBeforeKill: crash.confirmedClosedBeforeKill,
    decodedAfterRestart: crash.decodedAfterRestart,
    recoveries: [...crash.recoveries],
    hashesPreserved: crash.hashesPreserved,
  }
}

export function dryRunReport(options) {
  return {
    schemaVersion: 1,
    status: "partial_native_not_run",
    nativeExecution: false,
    reason: "Dry run validates arguments and artifact contract only; it makes no OBS or Ubuntu claim.",
    inputs: {
      pluginStage: options.pluginDirectory.split("/").at(-1),
      evidenceDirectory: "<private>",
      publisherHost: options.publisherHost,
      deadlineSeconds: options.deadlineSeconds,
    },
    expectedArtifacts: [
      "private/operator-auth.json", "private/*.publisher.json", "private/obs-fixture.json",
      "private/obs-fixture-control.json", "private/obs-fixture-report.json", "rehearsal-report.json",
    ],
  }
}
