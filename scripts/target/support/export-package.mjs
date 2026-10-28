import { constants } from 'node:fs';
import { chmod, copyFile, mkdir, rm, stat, writeFile } from 'node:fs/promises';
import path from 'node:path';

import { validateEditorPackage } from './export-contract.mjs';
import { sha256File, verifyMediaFile } from './export-media.mjs';

export async function buildPortablePackage(input, outputDirectory, tools) {
  await mkdir(outputDirectory, { mode: 0o700 });
  await mkdir(path.join(outputDirectory, 'metadata'), { mode: 0o700 });
  await mkdir(path.join(outputDirectory, 'media'), { mode: 0o700 });
  const packageCopy = redactSensitive(structuredClone(input));
  const validation = validateEditorPackage(input);
  const manifest = { schemaVersion: 1, media: [] };
  const structuralIssues = [...validation.errors];
  const decoderFiles = [];
  const relativeById = new Map();
  for (const [index, item] of input.mediaInventory.entries()) {
    const recording = item.recording;
    const checked = await checkAndCopy(recording, index, outputDirectory, tools);
    manifest.media.push(checked.manifest);
    if (checked.structuralIssue) structuralIssues.push(checked.structuralIssue);
    if (checked.decoder) decoderFiles.push({ recordingId: recording.id, ...checked.decoder });
    if (checked.relativePath) relativeById.set(recording.id, checked.relativePath);
  }
  rewriteRecordingPaths(packageCopy, relativeById);
  const decoderStatus = decoderFiles.some(file => file.status === 'failed')
    ? 'failed'
    : decoderFiles.some(file => file.status === 'passed') ? 'passed' : 'notRun';
  const report = {
    schemaVersion: 1,
    statuses: {
      structural: {
        status: structuralIssues.length === 0 ? 'passed' : 'failed',
        issues: [...new Set(structuralIssues)],
        warnings: validation.warnings,
      },
      decoder: { status: decoderStatus, files: decoderFiles },
      applicationImport: {
        status: 'notRun',
        reason: 'targetEditorUnspecified',
        application: null,
      },
    },
    reconstruction: input.reconstruction,
    limitations: limitations(input, decoderStatus),
    artifact: {
      metadata: 'metadata/editor-package.json',
      manifest: 'manifest.json',
      mediaDirectory: 'media',
    },
  };
  await writeJson(path.join(outputDirectory, 'metadata', 'editor-package.json'), packageCopy);
  await writeJson(path.join(outputDirectory, 'metadata', 'compatibility-report.json'), report);
  await writeJson(path.join(outputDirectory, 'manifest.json'), manifest);
  return { package: packageCopy, report, manifest, outputDirectory };
}

async function checkAndCopy(recording, index, outputDirectory, tools) {
  const manifest = {
    recordingId: recording.id,
    relativePath: null,
    copied: false,
    sha256: recording.sha256 ?? null,
    bytes: recording.bytes,
    recoveryStatus: recording.recoveryStatus,
    mappingStatus: recording.mappingStatus,
  };
  let file;
  try {
    file = await stat(recording.path);
  } catch {
    return { manifest, relativePath: null, structuralIssue: `recordingMissing:${recording.id}`, decoder: null };
  }
  if (!file.isFile()) return { manifest, relativePath: null, structuralIssue: `recordingNotFile:${recording.id}`, decoder: null };
  const actualHash = await sha256File(recording.path);
  if (recording.sha256 === null || recording.sha256 === undefined) {
    return { manifest, relativePath: null, structuralIssue: `recordingChecksumMissing:${recording.id}`, decoder: null };
  }
  if (actualHash !== recording.sha256) {
    return { manifest, relativePath: null, structuralIssue: `recordingChecksumMismatch:${recording.id}`, decoder: null };
  }
  if (BigInt(recording.bytes) !== BigInt(file.size)) {
    return { manifest, relativePath: null, structuralIssue: `recordingSizeMismatch:${recording.id}`, decoder: null };
  }
  const decoder = await verifyMediaFile(recording, tools);
  if (decoder.status === 'failed') {
    decoder.detail = undefined;
    return { manifest, relativePath: null, structuralIssue: null, decoder };
  }
  const extension = safeExtension(recording.path);
  const filename = `${String(index).padStart(4, '0')}-${safeName(recording.id)}${extension}`;
  const relativePath = path.posix.join('media', filename);
  const destination = path.join(outputDirectory, relativePath);
  await copyFile(recording.path, destination, constants.COPYFILE_EXCL);
  await chmod(destination, 0o600);
  const copiedHash = await sha256File(destination);
  const copiedSize = (await stat(destination)).size;
  if (copiedHash !== actualHash || BigInt(copiedSize) !== BigInt(recording.bytes)) {
    await rm(destination, { force: true });
    return { manifest, relativePath: null, structuralIssue: `copiedMediaIntegrityFailure:${recording.id}`, decoder };
  }
  manifest.relativePath = relativePath;
  manifest.copied = true;
  manifest.sha256 = actualHash;
  return { manifest, relativePath, structuralIssue: null, decoder };
}

function rewriteRecordingPaths(value, relativeById) {
  for (const item of value.mediaInventory) {
    item.recording.path = relativeById.get(item.recording.id) ?? null;
  }
  if (value.programReference !== null) {
    value.programReference.path = relativeById.get(value.programReference.id) ?? null;
  }
}

function redactSensitive(value) {
  if (Array.isArray(value)) return value.map(redactSensitive);
  if (value === null || typeof value !== 'object') return value;
  const result = {};
  for (const [key, child] of Object.entries(value)) {
    result[key] = /(passphrase|password|secret|token|cookie|credential|streamid|authorization|url|uri|endpoint|destination)/i.test(key)
      ? '[REDACTED]'
      : redactSensitive(child);
  }
  return result;
}

function limitations(input, decoderStatus) {
  const values = ['targetEditorUnspecified', 'applicationImportNotRun'];
  if (input.timebase?.mapping === null) values.push('clockMappingUnavailable');
  if (input.programReference === null) values.push('programReferenceUnavailable');
  if (input.reconstruction !== 'editable') values.push(`reconstruction:${input.reconstruction}`);
  if (decoderStatus !== 'passed') values.push('notAllMediaDecodeValidated');
  return values;
}

function safeName(value) {
  return value.replace(/[^A-Za-z0-9._-]/g, '_').slice(0, 120) || 'recording';
}
function safeExtension(filename) {
  const extension = path.extname(filename).toLowerCase();
  return /^\.[a-z0-9]{1,10}$/.test(extension) ? extension : '.bin';
}
async function writeJson(filename, value) {
  await writeFile(filename, `${JSON.stringify(value, null, 2)}\n`, { mode: 0o600 });
}
