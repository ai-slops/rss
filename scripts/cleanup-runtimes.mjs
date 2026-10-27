import assert from 'node:assert/strict';
import { nativeInstances } from './native-processes.mjs';

assert.ok(process.argv.includes('--all'), 'Stopping all project native runtimes requires explicit --all authorization');
const before = await nativeInstances();
console.info(JSON.stringify({ before }));
const deadline = Date.now() + 10000;
for (;;) {
  const active = await nativeInstances();
  if (active.length === 0) break;
  for (const processIdentity of active) {
    try { process.kill(processIdentity.pid, Date.now() < deadline - 5000 ? 'SIGTERM' : 'SIGKILL'); }
    catch (error) { if (!(error instanceof Error) || !('code' in error) || error.code !== 'ESRCH') throw error; }
  }
  assert.ok(Date.now() < deadline, 'Project runtimes persisted after bounded TERM/KILL cleanup');
  await new Promise(resolve => setTimeout(resolve, 50));
}
console.info(JSON.stringify({ nativeBefore: before.length, nativeAfter: 0 }));
