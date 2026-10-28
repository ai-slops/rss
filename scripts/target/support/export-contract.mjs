const identifier = /^[A-Za-z0-9._:-]{1,256}$/;
const decimal = /^(0|[1-9][0-9]*)$/;
const checksum = /^[a-f0-9]{64}$/;
const recoveryStates = new Set(['complete', 'recovered', 'partial']);
const mappingStates = new Set(['mapped', 'unmapped']);

export function validateEditorPackage(value) {
  const errors = [];
  const warnings = [];
  if (!isObject(value)) return { valid: false, errors: ['packageMustBeObject'], warnings };
  requireInteger(value.schemaVersion, 'schemaVersion', errors);
  requireIdentifier(value.sessionId, 'sessionId', errors);
  requireIdentifier(value.projectId, 'projectId', errors);
  requireIdentifier(value.engineRunId, 'engineRunId', errors);
  for (const field of ['mediaInventory', 'layoutUse', 'audioHistory', 'timingHistory', 'markers', 'gaps', 'issues']) {
    if (!Array.isArray(value[field])) errors.push(`${field}MustBeArray`);
  }
  validateTimebase(value.timebase, value.engineRunId, errors, warnings);
  const seen = new Set();
  const media = Array.isArray(value.mediaInventory) ? value.mediaInventory : [];
  for (const [index, item] of media.entries()) validateMedia(item, index, seen, errors, warnings);
  validateProgramReference(value, seen, errors, warnings);
  if (!['editable', 'recorded-layer', 'approximate', 'program-only'].includes(value.reconstruction)) {
    errors.push('reconstructionCapabilityUnknown');
  }
  return { valid: errors.length === 0, errors, warnings: [...new Set(warnings)] };
}

function validateMedia(item, index, seen, errors, warnings) {
  const prefix = `mediaInventory[${index}]`;
  if (!isObject(item) || !isObject(item.recording)) {
    errors.push(`${prefix}.recordingMissing`);
    return;
  }
  const recording = item.recording;
  if (!requireIdentifier(recording.id, `${prefix}.recording.id`, errors)) return;
  if (seen.has(recording.id)) errors.push(`${prefix}.recording.idDuplicate`);
  seen.add(recording.id);
  requireIdentifier(recording.outputId, `${prefix}.recording.outputId`, errors);
  if (typeof recording.path !== 'string' || recording.path.length === 0) errors.push(`${prefix}.recording.pathMissing`);
  if (!recoveryStates.has(recording.recoveryStatus)) errors.push(`${prefix}.recoveryStatusUnknown`);
  if (!mappingStates.has(recording.mappingStatus)) errors.push(`${prefix}.mappingStatusUnknown`);
  if (!decimal.test(recording.bytes ?? '')) errors.push(`${prefix}.bytesNotDecimalString`);
  if (recording.sha256 !== null && recording.sha256 !== undefined && !checksum.test(recording.sha256)) errors.push(`${prefix}.sha256Invalid`);
  validateRecordingTimes(recording, prefix, errors, warnings);
  validateProvenance(recording, item, prefix, errors, warnings);
  validateFileMapping(recording, item.fileTimeMapping, prefix, errors, warnings);
  if (recording.recoveryStatus === 'partial' || recording.validation !== 'decoded') warnings.push('partialOrUnvalidatedRecording');
  if (!Array.isArray(recording.gaps)) errors.push(`${prefix}.gapsMustBeArray`);
}

function validateRecordingTimes(recording, prefix, errors, warnings) {
  if (recording.startNs !== 'unknown' && !decimal.test(recording.startNs ?? '')) errors.push(`${prefix}.startNsInvalid`);
  optionalDecimal(recording.endNs, `${prefix}.endNs`, errors);
  optionalDecimal(recording.mediaStartNs, `${prefix}.mediaStartNs`, errors);
  optionalDecimal(recording.mediaEndNs, `${prefix}.mediaEndNs`, errors);
  optionalDecimal(recording.mediaUncertaintyNs, `${prefix}.mediaUncertaintyNs`, errors);
  if (recording.mappingStatus === 'mapped') {
    requireIdentifier(recording.engineRunId, `${prefix}.engineRunId`, errors);
    const start = requireDecimal(recording.mediaStartNs, `${prefix}.mediaStartNs`, errors);
    const end = requireDecimal(recording.mediaEndNs, `${prefix}.mediaEndNs`, errors);
    requireDecimal(recording.mediaUncertaintyNs, `${prefix}.mediaUncertaintyNs`, errors);
    if (start !== null && end !== null && end < start) errors.push(`${prefix}.mediaRangeReversed`);
  } else {
    warnings.push('recordingTimebaseUnmapped');
  }
}

function validateProvenance(recording, item, prefix, errors, warnings) {
  const intervals = Array.isArray(recording.provenance) ? recording.provenance : [];
  const gaps = Array.isArray(recording.provenanceGaps) ? recording.provenanceGaps : [];
  if (!Array.isArray(recording.provenance)) errors.push(`${prefix}.provenanceMustBeArray`);
  if (!Array.isArray(recording.provenanceGaps)) errors.push(`${prefix}.provenanceGapsMustBeArray`);
  const generations = new Set();
  const sources = new Set();
  const fileStart = parseDecimal(recording.mediaStartNs);
  const fileEnd = parseDecimal(recording.mediaEndNs);
  for (const [position, interval] of intervals.entries()) {
    const name = `${prefix}.provenance[${position}]`;
    requireIdentifier(interval.engine_run_id, `${name}.engineRunId`, errors);
    requireIdentifier(interval.source_id, `${name}.sourceId`, errors);
    requireIdentifier(interval.track_id, `${name}.trackId`, errors);
    const localStart = requireDecimal(interval.local_start_ns, `${name}.localStartNs`, errors);
    const localEnd = requireDecimal(interval.local_end_ns, `${name}.localEndNs`, errors);
    const mediaStart = requireDecimal(interval.media_start_ns, `${name}.mediaStartNs`, errors);
    const mediaEnd = requireDecimal(interval.media_end_ns, `${name}.mediaEndNs`, errors);
    if (localStart !== null && localEnd !== null && localEnd < localStart) errors.push(`${name}.localRangeReversed`);
    if (mediaStart !== null && mediaEnd !== null && mediaEnd < mediaStart) errors.push(`${name}.mediaRangeReversed`);
    if (fileStart !== null && mediaStart !== null && mediaStart < fileStart) errors.push(`${name}.startsBeforeFile`);
    if (fileEnd !== null && mediaEnd !== null && mediaEnd > fileEnd) errors.push(`${name}.endsAfterFile`);
    if (interval.engine_run_id !== recording.engineRunId) errors.push(`${name}.engineRunMismatch`);
    if (!validGeneration(interval.continuity_generation)) errors.push(`${name}.generationInvalid`);
    generations.add(String(interval.continuity_generation));
    sources.add(interval.source_id);
  }
  for (const [position, gap] of gaps.entries()) validateGap(gap, `${prefix}.provenanceGaps[${position}]`, fileStart, fileEnd, errors);
  if (generations.size > 1) {
    warnings.push('mixedContinuityGenerations');
    if (item.sourceContinuityGeneration !== null) errors.push(`${prefix}.mixedGenerationMustNotCollapse`);
  }
  if (sources.size === 1 && item.sourceId !== null && item.sourceId !== [...sources][0]) errors.push(`${prefix}.sourceIdMismatch`);
  if (recording.tapPoint === 'clean_source' && intervals.length === 0) warnings.push('cleanSourceProvenanceUnknown');
  if (recording.tapPoint === 'program' && intervals.length !== 0) errors.push(`${prefix}.programMustNotClaimSourceProvenance`);
}

function validateGap(gap, prefix, fileStart, fileEnd, errors) {
  requireIdentifier(gap.source_id, `${prefix}.sourceId`, errors);
  requireIdentifier(gap.track_id, `${prefix}.trackId`, errors);
  const localStart = requireDecimal(gap.local_start_ns, `${prefix}.localStartNs`, errors);
  const localEnd = requireDecimal(gap.local_end_ns, `${prefix}.localEndNs`, errors);
  const mediaStart = requireDecimal(gap.media_start_ns, `${prefix}.mediaStartNs`, errors);
  const mediaEnd = requireDecimal(gap.media_end_ns, `${prefix}.mediaEndNs`, errors);
  if (localStart !== null && localEnd !== null && localEnd < localStart) errors.push(`${prefix}.localRangeReversed`);
  if (mediaStart !== null && mediaEnd !== null && mediaEnd < mediaStart) errors.push(`${prefix}.mediaRangeReversed`);
  if (fileStart !== null && mediaStart !== null && mediaStart < fileStart) errors.push(`${prefix}.startsBeforeFile`);
  if (fileEnd !== null && mediaEnd !== null && mediaEnd > fileEnd) errors.push(`${prefix}.endsAfterFile`);
  if (!validGeneration(gap.from_generation) || !validGeneration(gap.to_generation)) errors.push(`${prefix}.generationInvalid`);
}

function validateFileMapping(recording, mapping, prefix, errors, warnings) {
  if (mapping === null || mapping === undefined) {
    if (recording.mappingStatus === 'mapped') errors.push(`${prefix}.fileTimeMappingMissing`);
    else warnings.push('fileTimeMappingUnavailable');
    return;
  }
  if (!isObject(mapping)) return errors.push(`${prefix}.fileTimeMappingInvalid`);
  if (mapping.engineRunId !== recording.engineRunId) errors.push(`${prefix}.fileTimeMappingRunMismatch`);
  for (const field of ['mediaStartNs', 'mediaEndNs', 'wallStartNs', 'wallEndNs', 'uncertaintyNs']) requireDecimal(mapping[field], `${prefix}.fileTimeMapping.${field}`, errors);
  if (mapping.mediaStartNs !== recording.mediaStartNs || mapping.mediaEndNs !== recording.mediaEndNs) errors.push(`${prefix}.fileTimeMappingBoundsMismatch`);
}

function validateTimebase(timebase, engineRunId, errors, warnings) {
  if (!isObject(timebase)) return errors.push('timebaseMissing');
  if (timebase.mediaClockDomain !== 'engineRunPtsNs') errors.push('mediaClockDomainUnknown');
  if (timebase.wallClockDomain !== 'unixEpochNs') errors.push('wallClockDomainUnknown');
  if (timebase.mapping === null) return warnings.push('clockMappingUnavailable');
  if (!isObject(timebase.mapping)) return errors.push('clockMappingInvalid');
  if (timebase.mapping.engineRunId !== engineRunId) errors.push('clockMappingEngineRunMismatch');
  for (const field of ['mediaPtsNs', 'wallTimeNs', 'boundNs']) requireDecimal(timebase.mapping[field], `timebase.mapping.${field}`, errors);
}

function validateProgramReference(value, seen, errors, warnings) {
  if (value.programReference === null) {
    warnings.push('programReferenceUnavailable');
    if (!value.issues?.some(issue => issue?.kind === 'missingProgramReference')) errors.push('missingProgramReferenceIssueRequired');
    return;
  }
  if (!isObject(value.programReference) || !seen.has(value.programReference.id)) errors.push('programReferenceNotInInventory');
}

function requireIdentifier(value, field, errors) {
  if (typeof value !== 'string' || !identifier.test(value)) { errors.push(`${field}Invalid`); return false; }
  return true;
}
function requireInteger(value, field, errors) { if (!Number.isSafeInteger(value)) errors.push(`${field}MustBeSafeInteger`); }
function requireDecimal(value, field, errors) { if (typeof value !== 'string' || !decimal.test(value)) { errors.push(`${field}MustBeDecimalString`); return null; } return BigInt(value); }
function optionalDecimal(value, field, errors) { if (value !== null && value !== undefined) requireDecimal(value, field, errors); }
function parseDecimal(value) { return typeof value === 'string' && decimal.test(value) ? BigInt(value) : null; }
function validGeneration(value) { return Number.isSafeInteger(value) && value >= 0 || typeof value === 'string' && decimal.test(value); }
function isObject(value) { return value !== null && typeof value === 'object' && !Array.isArray(value); }
