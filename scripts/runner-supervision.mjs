import { spawn } from 'node:child_process';
import { readFile } from 'node:fs/promises';

export async function ensureSupervised() {
  const owner = process.env.LIVE_PRODUCTION_OWNER_PID;
  const expected = process.env.LIVE_PRODUCTION_OWNER_START_TICKS;
  if (owner !== undefined && expected !== undefined) {
    const stat = await readFile(`/proc/${owner}/stat`, 'utf8');
    if (stat.slice(stat.lastIndexOf(') ') + 2).trim().split(/\s+/)[19] !== expected) {
      throw new Error('The smoke supervisor no longer owns this process generation');
    }
    return;
  }
  const executable = new URL('../target/debug/live-production-supervisor', import.meta.url).pathname;
  const child = spawn(executable, ['--timeout-ms', '240000', '--grace-ms', '10000', '--', process.execPath, ...process.argv.slice(1)], { stdio: 'inherit' });
  const forward = signal => child.kill(signal);
  const handlers = new Map(['SIGTERM', 'SIGINT', 'SIGHUP'].map(signal => [signal, () => forward(signal)]));
  for (const [signal, handler] of handlers) process.on(signal, handler);
  try {
    const result = await new Promise((resolve, reject) => {
      child.once('error', reject);
      child.once('exit', code => resolve(code ?? 1));
    });
    process.exit(result);
  } finally {
    for (const [signal, handler] of handlers) process.off(signal, handler);
    child.kill('SIGTERM');
  }
}
