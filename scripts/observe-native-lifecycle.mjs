import { spawn } from 'node:child_process';
import { mkdir, open, readFile, readdir, readlink, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { ensureSupervised } from './runner-supervision.mjs';

const root = resolve(new URL('..', import.meta.url).pathname);
if (process.argv[2] === '--help') {
  console.log('Usage: node scripts/observe-native-lifecycle.mjs NEW_PRIVATE_EVIDENCE_DIRECTORY\nRuns unchanged lifecycle acceptance and records read-only process-generation/state transitions.');
  process.exit(0);
}
if (process.argv.length !== 3) throw new Error('Usage: node scripts/observe-native-lifecycle.mjs NEW_PRIVATE_EVIDENCE_DIRECTORY');
await ensureSupervised();
const directory = resolve(process.argv[2]);
await mkdir(directory, { mode: 0o700 });
const log = await open(`${directory}/lifecycle.log`, 'wx', 0o600);
const tracked = new Map();
const observations = [];
const child = spawn('bash', [`${root}/scripts/lifecycle-acceptance.sh`], {
  cwd: root,
  env: { ...process.env, LIVE_PRODUCTION_LIFECYCLE_EXPECT_ZERO: '1' },
  stdio: ['ignore', log.fd, log.fd],
});
await log.close();
let ended = false;
const completion = new Promise((resolveExit, reject) => {
  child.once('exit', (code, signal) => {
    ended = true;
    resolveExit({ code, signal });
  });
  child.once('error', error => {
    ended = true;
    reject(error);
  });
});
try {
  while (!ended) {
    await sample();
    await delay(10);
  }
  await sample();
  const outcome = await completion;
  await writeFile(`${directory}/observations.json`, `${JSON.stringify({
    schemaVersion: 1,
    observerPolicy: 'read-only proc identity/state; no arguments, credentials or signal interception',
    outcome,
    observations,
  }, null, 2)}\n`, { mode: 0o600, flag: 'wx' });
  console.log(JSON.stringify({ outcome, processes: tracked.size, zombieTransitions: observations.filter(value => value.state === 'Z').length }));
  process.exitCode = outcome.code ?? 1;
} finally {
  if (!ended) child.kill('SIGTERM');
}

async function sample() {
  const present = new Set();
  for (const entry of await readdir('/proc')) {
    if (!/^\d+$/u.test(entry)) continue;
    try {
      const stat = await readFile(`/proc/${entry}/stat`, 'utf8');
      const fields = stat.slice(stat.lastIndexOf(') ') + 2).trim().split(/\s+/u);
      const key = `${entry}:${fields[19]}`;
      let identity = tracked.get(key);
      if (identity === undefined) {
        const executable = await readlink(`/proc/${entry}/exe`);
        if (!executable.startsWith(`${root}/`) || !executable.replace(/ \(deleted\)$/u, '').endsWith('/live-production')) continue;
        identity = { pid: Number(entry), startTicks: fields[19], lastState: null, lastParent: null };
        tracked.set(key, identity);
      }
      present.add(key);
      const state = fields[0];
      const parentPid = Number(fields[1]);
      if (state === identity.lastState && parentPid === identity.lastParent) continue;
      observations.push({ time: new Date().toISOString(), pid: identity.pid, startTicks: identity.startTicks, state, parentPid });
      identity.lastState = state;
      identity.lastParent = parentPid;
    } catch (error) {
      if (!(error instanceof Error) || !['ENOENT', 'ESRCH', 'EACCES'].includes(error.code)) throw error;
    }
  }
  for (const [key, identity] of tracked) {
    if (present.has(key) || identity.lastState === 'gone') continue;
    observations.push({ time: new Date().toISOString(), pid: identity.pid, startTicks: identity.startTicks, state: 'gone', parentPid: null });
    identity.lastState = 'gone';
    identity.lastParent = null;
  }
}
