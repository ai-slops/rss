import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { mkdir, mkdtemp, readFile, stat, writeFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';
import { promisify } from 'node:util';

import { validateEditorPackage } from './support/export-contract.mjs';
import { runSupervised } from './support/export-media.mjs';
import { buildPortablePackage } from './support/export-package.mjs';

const supervisor = process.env.LIVE_PRODUCTION_SUPERVISOR;
const ffmpeg = process.env.FFMPEG;
const ffprobe = process.env.FFPROBE;
const execute = promisify(execFile);
const projectRoot = path.resolve(path.dirname(new URL(import.meta.url).pathname), '..', '..');
const privateRoot = path.join(projectRoot, 'var', 'target');

test('real media package passes structural integrity probe and full decode', {
  skip: !supervisor || !ffmpeg || !ffprobe,
}, async () => {
  const root = await privateDirectory('export-compat-real-');
  const media = path.join(root, 'program.mkv');
  await runSupervised(supervisor, ffmpeg, [
    '-v', 'error', '-f', 'lavfi', '-i', 'testsrc=size=160x90:rate=10:duration=1',
    '-f', 'lavfi', '-i', 'sine=frequency=440:sample_rate=48000:duration=1',
    '-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-c:a', 'aac', '-shortest', media,
  ], 30_000);
  const input = await packageFixture(media, { program: true });
  input.sourceConfigs = [{ sourceId: 'source-1', passphrase: 'do-not-export', streamId: 'scoped-secret', endpointUrl: 'srt://private/key' }];
  const output = path.join(root, 'portable');
  const result = await buildPortablePackage(input, output, { supervisor, ffmpeg, ffprobe });
  assert.equal(result.report.statuses.structural.status, 'passed', JSON.stringify(result.report.statuses.structural));
  assert.equal(result.report.statuses.decoder.status, 'passed');
  assert.equal(result.report.statuses.applicationImport.status, 'notRun');
  assert.equal(result.report.statuses.applicationImport.reason, 'targetEditorUnspecified');
  assert.equal(result.package.sourceConfigs[0].passphrase, '[REDACTED]');
  assert.equal(result.package.sourceConfigs[0].streamId, '[REDACTED]');
  assert.equal(result.package.sourceConfigs[0].endpointUrl, '[REDACTED]');
  assert.equal(input.sourceConfigs[0].passphrase, 'do-not-export');
  assert.equal(JSON.stringify(result.package).includes(media), false);
  assert.equal(JSON.stringify(result.report).includes(media), false);
  assert.equal(JSON.stringify(result.report).includes('do-not-export'), false);
  assert.equal(await sha256File(media), input.mediaInventory[0].recording.sha256);
  assert.match(result.package.mediaInventory[0].recording.path, /^media\//);
  assert.equal(result.package.mediaInventory[0].recording.startNs, '9007199254740993');
  assert.equal((await stat(path.join(output, result.package.mediaInventory[0].recording.path))).size > 0, true);
  assert.equal(JSON.parse(await readFile(path.join(output, 'manifest.json'), 'utf8')).media.length, 1);
  const inputFile = path.join(root, 'editor-package.json');
  const cliOutput = path.join(root, 'portable-cli');
  await writeFile(inputFile, JSON.stringify(input));
  const cli = await execute(process.execPath, [
    new URL('./export-compatibility.mjs', import.meta.url).pathname,
    '--input', inputFile, '--output', cliOutput,
    '--supervisor', supervisor, '--ffprobe', ffprobe, '--ffmpeg', ffmpeg,
  ], { timeout: 180_000 });
  assert.equal(JSON.parse(cli.stdout).applicationImport, 'notRun');
});

test('hash mismatch is structural failure and media is not copied', async () => {
  const root = await privateDirectory('export-compat-hash-');
  const media = path.join(root, 'bad.mkv');
  await writeFile(media, 'not-media');
  const input = await packageFixture(media, { sha256: '0'.repeat(64), program: true });
  const result = await buildPortablePackage(input, path.join(root, 'portable'), {
    supervisor: supervisor ?? '/missing-supervisor', ffmpeg: ffmpeg ?? '/missing-ffmpeg', ffprobe: ffprobe ?? '/missing-ffprobe',
  });
  assert.equal(result.report.statuses.structural.status, 'failed');
  assert.equal(result.report.statuses.decoder.status, 'notRun');
  assert.equal(result.manifest.media[0].copied, false);
});

test('missing clock and mixed generations remain structurally readable but approximate', async () => {
  const input = await packageFixture('/tmp/unused.mkv', { program: false, mixedGenerations: true });
  input.timebase.mapping = null;
  input.issues = [{ kind: 'missingClockMapping' }, { kind: 'missingProgramReference' }];
  const validation = validateEditorPackage(input);
  assert.equal(validation.valid, true, JSON.stringify(validation));
  assert(validation.warnings.includes('clockMappingUnavailable'));
  assert(validation.warnings.includes('mixedContinuityGenerations'));
  assert.equal(input.mediaInventory[0].sourceContinuityGeneration, null);
});

test('partial recovery is never promoted to decode validated clip', async () => {
  const root = await privateDirectory('export-compat-partial-');
  const media = path.join(root, 'partial.mkv');
  await writeFile(media, 'partial-tail');
  const input = await packageFixture(media, { recoveryStatus: 'partial', validation: 'partial' });
  const result = await buildPortablePackage(input, path.join(root, 'portable'), {
    supervisor: supervisor ?? '/missing-supervisor', ffmpeg: ffmpeg ?? '/missing-ffmpeg', ffprobe: ffprobe ?? '/missing-ffprobe',
  });
  assert.equal(result.report.statuses.decoder.status, 'failed');
  assert.equal(result.report.statuses.decoder.files[0].reason, 'recordingNotDecodeValidated');
});

test('missing media remains explicit and is never packaged as a clip', async () => {
  const root = await privateDirectory('export-compat-missing-');
  const input = await packageFixture(path.join(root, 'missing.mkv'), { sha256: '1'.repeat(64), program: true });
  const result = await buildPortablePackage(input, path.join(root, 'portable'), {
    supervisor: supervisor ?? '/missing-supervisor', ffmpeg: ffmpeg ?? '/missing-ffmpeg', ffprobe: ffprobe ?? '/missing-ffprobe',
  });
  assert.equal(result.report.statuses.structural.status, 'failed');
  assert(result.report.statuses.structural.issues.includes('recordingMissing:recording-1'));
  assert.equal(result.manifest.media[0].copied, false);
});

async function packageFixture(mediaPath, options = {}) {
  const sha256 = options.sha256 ?? await sha256File(mediaPath).catch(() => '1'.repeat(64));
  const generations = options.program === false ? (options.mixedGenerations ? [1, 2] : [1]) : [];
  const recording = {
    id: 'recording-1', outputId: 'output-1', name: 'Program', path: mediaPath,
    tapPoint: options.program === false ? 'clean_source' : 'program', sessionId: 'session-1',
    recoveryStatus: options.recoveryStatus ?? 'complete', mappingStatus: 'mapped',
    startNs: '9007199254740993', endNs: '9007199255740993', engineRunId: 'run-1',
    mediaStartNs: '1000000000', mediaEndNs: '2000000000', mediaUncertaintyNs: '33333333',
    bytes: String((await stat(mediaPath).catch(() => ({ size: 0 }))).size), validation: options.validation ?? 'decoded', sha256, gaps: [],
    provenance: generations.map((generation, index) => ({
      engine_run_id: 'run-1', source_id: 'source-1', track_id: 'video-1', track_kind: 'video',
      continuity_generation: generation, local_start_ns: String(index * 500000000), local_end_ns: String((index + 1) * 500000000),
      media_start_ns: String(1000000000 + index * 500000000), media_end_ns: String(1500000000 + index * 500000000),
    })), provenanceGaps: options.mixedGenerations ? [{
      source_id: 'source-1', track_id: 'video-1', from_generation: 1, to_generation: 2,
      local_start_ns: '500000000', local_end_ns: '500000000', media_start_ns: '1500000000', media_end_ns: '1500000000',
    }] : [],
  };
  const media = { recording, tapPoint: recording.tapPoint, sourceId: options.program === false ? 'source-1' : null,
    sourceContinuityGeneration: generations.length === 1 ? String(generations[0]) : null,
    intervalClockDomain: 'engineRunPtsNs', fileTimeMapping: { engineRunId: 'run-1', mediaStartNs: '1000000000', mediaEndNs: '2000000000', wallStartNs: '9007199254740993', wallEndNs: '9007199255740993', uncertaintyNs: '33333334' } };
  return { schemaVersion: 1, sessionId: 'session-1', projectId: 'project-1', engineRunId: 'run-1', mediaInventory: [media],
    programReference: options.program === false ? null : recording, layoutUse: [], audioHistory: [], timingHistory: [], markers: [],
    timebase: { mediaClockDomain: 'engineRunPtsNs', wallClockDomain: 'unixEpochNs', mapping: { engineRunId: 'run-1', mediaPtsNs: '1', wallTimeNs: '9007199254740993', boundNs: '1' } }, gaps: [], issues: options.program === false ? [{ kind: 'missingProgramReference' }] : [], reconstruction: 'approximate' };
}

async function sha256File(filename) {
  const { createHash } = await import('node:crypto');
  return createHash('sha256').update(await readFile(filename)).digest('hex');
}

async function privateDirectory(prefix) {
  await mkdir(privateRoot, { recursive: true, mode: 0o700 });
  return mkdtemp(path.join(privateRoot, prefix));
}
