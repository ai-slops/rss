import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';

const maximumOutputBytes = 4 * 1024 * 1024;

export async function runSupervised(supervisor, executable, args, timeoutMs = 60_000) {
  const child = spawn(supervisor, [
    '--timeout-ms', String(timeoutMs), '--grace-ms', '5000', '--', executable, ...args,
  ], { stdio: ['ignore', 'pipe', 'pipe'] });
  let stdout = '';
  let stderr = '';
  child.stdout.on('data', chunk => { stdout = boundedAppend(stdout, chunk); });
  child.stderr.on('data', chunk => { stderr = boundedAppend(stderr, chunk); });
  const result = await new Promise((resolve, reject) => {
    child.once('error', reject);
    child.once('exit', (code, signal) => resolve({ code: code ?? 1, signal }));
  });
  if (result.code !== 0) {
    const error = new Error(`supervised command failed with code ${result.code}`);
    error.result = { ...result, stdout, stderr };
    throw error;
  }
  return { ...result, stdout, stderr };
}

export async function sha256File(filename) {
  const digest = createHash('sha256');
  for await (const chunk of createReadStream(filename)) digest.update(chunk);
  return digest.digest('hex');
}

export async function verifyMediaFile(recording, tools) {
  if (recording.recoveryStatus === 'partial' || recording.validation !== 'decoded') {
    return { status: 'failed', reason: 'recordingNotDecodeValidated', probe: null, fullDecode: 'notRun' };
  }
  let probe;
  try {
    const result = await runSupervised(tools.supervisor, tools.ffprobe, [
      '-v', 'error', '-show_streams', '-show_format', '-of', 'json', recording.path,
    ], 30_000);
    probe = JSON.parse(result.stdout);
  } catch (error) {
    return { status: 'failed', reason: 'ffprobeFailed', detail: childFailure(error), probe: null, fullDecode: 'notRun' };
  }
  const streams = Array.isArray(probe.streams) ? probe.streams : [];
  const negotiated = streams.map(stream => ({
    index: stream.index,
    type: stream.codec_type,
    codec: stream.codec_name,
    timeBase: stream.time_base ?? null,
  }));
  if (!negotiated.some(stream => stream.type === 'video') || !negotiated.some(stream => stream.type === 'audio')) {
    return { status: 'failed', reason: 'requiredStreamsMissing', probe: { streams: negotiated }, fullDecode: 'notRun' };
  }
  try {
    await runSupervised(tools.supervisor, tools.ffmpeg, [
      '-v', 'error', '-xerror', '-i', recording.path, '-map', '0', '-f', 'null', '-',
    ], 120_000);
  } catch (error) {
    return { status: 'failed', reason: 'fullDecodeFailed', detail: childFailure(error), probe: { streams: negotiated }, fullDecode: 'failed' };
  }
  return { status: 'passed', reason: null, probe: { streams: negotiated }, fullDecode: 'passed' };
}

function boundedAppend(current, chunk) {
  if (current.length >= maximumOutputBytes) return current;
  return (current + chunk.toString('utf8')).slice(0, maximumOutputBytes);
}

function childFailure(error) {
  if (!(error instanceof Error)) return 'unknown child failure';
  const stderr = error.result?.stderr?.trim();
  return stderr ? stderr.slice(0, 2_000) : error.message;
}
