import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp } from 'node:fs/promises';
import { once } from 'node:events';
import { createInterface } from 'node:readline';
import { ensureSupervised } from './runner-supervision.mjs';
import { nativeInstances } from './native-processes.mjs';

await ensureSupervised();
const root = new URL('../', import.meta.url).pathname.replace(/\/$/, '');
const executable = `${root}/target/debug/live-production-supervisor`;
const fixture = new URL('./test-fixtures/native-smoke.mjs', import.meta.url).pathname;

test('repeated native smoke outcomes return live and obsolete executables to baseline', { timeout: 90000 }, async () => {
  const baseline = await nativeInstances();
  if (process.env.LIVE_PRODUCTION_LIFECYCLE_EXPECT_ZERO === '1') assert.equal(baseline.length, 0, 'zero-baseline lifecycle gate requires all previews stopped');
  const identities = values => values.map(({ pid, generation, deleted }) => ({ pid, generation, deleted }));
  const cases = [
    ['success', null, 0], ['failure', null, 37], ['deleted', null, 0], ['timeout', null, 124],
    ['signal', 'SIGINT', 130], ['signal', 'SIGTERM', 143], ['signal', 'SIGHUP', 129],
    ['success', null, 0], ['failure', null, 37], ['deleted', null, 0],
    ['owner-loss', 'SIGKILL', null],
  ];
  for (const [mode, signal, expected] of cases) {
    const state = await mkdtemp(`${root}/var/lifecycle-`);
    const child = spawn(executable, ['--timeout-ms', mode === 'timeout' ? '2000' : '15000', '--grace-ms', '3000', '--', process.execPath, fixture, mode, state], { stdio: ['ignore', 'pipe', 'inherit'] });
    const exited = once(child, 'exit');
    const lines = createInterface({ input: child.stdout });
    const watchdog = setTimeout(() => child.kill('SIGTERM'), 16000);
    try {
      const [line] = await Promise.race([once(lines, 'line'), exited.then(() => { throw new Error(`Native fixture exited before readiness: ${state}`); })]);
      const descriptor = JSON.parse(line);
      if (signal !== null) child.kill(signal);
      const [code] = await exited;
      assert.equal(code, expected, `native ${mode}/${signal ?? 'none'} did not preserve the outcome`);
      const deadline = Date.now() + 5000;
      while (JSON.stringify(identities(await nativeInstances())) !== JSON.stringify(identities(baseline)) && Date.now() < deadline) {
        await new Promise(resolve => setTimeout(resolve, 50));
      }
      assert.throws(() => process.kill(descriptor.appPid, 0), { code: 'ESRCH' });
      assert.deepEqual(identities(await nativeInstances()), identities(baseline), `${mode} leaked a live/deleted native executable`);
      console.info(`native lifecycle ${mode}/${signal ?? 'none'}: baseline restored`);
    } finally {
      clearTimeout(watchdog);
      lines.close();
      child.kill('SIGTERM');
      await exited;
    }
  }
  console.info(JSON.stringify({ runs: cases.length, nativeBefore: baseline.length, nativeAfter: (await nativeInstances()).length }));
});
