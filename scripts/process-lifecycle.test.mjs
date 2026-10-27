import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFile, spawn } from 'node:child_process';
import { promisify } from 'node:util';

const execute = promisify(execFile);
const supervisor = new URL('../target/debug/live-production-supervisor', import.meta.url).pathname;
const fixture = new URL('./test-fixtures/leaky-command.mjs', import.meta.url).pathname;

test('supervisor drains a descendant abandoned by a successful smoke command', async () => {
  const { stdout } = await execute(supervisor, ['--grace-ms', '100', '--', process.execPath, fixture, 'success'], { timeout: 4000 });
  assert.ok(stdout.includes('descendantPid'), 'supervisor must execute the requested fixture');
  const { descendantPid } = JSON.parse(stdout);
  assert.throws(() => process.kill(descendantPid, 0), { code: 'ESRCH' });
});

for (const mode of ['failure', 'escaped', 'timeout']) {
  test(`supervisor drains and reaps owned descendants on ${mode}`, async () => {
    let result;
    try {
      result = await execute(supervisor, ['--grace-ms', '100', '--timeout-ms', '600', '--', process.execPath, fixture, mode], { timeout: 4000 });
      assert.equal(mode, 'escaped');
    } catch (error) {
      assert.equal(error.code, mode === 'failure' ? 23 : 124);
      result = error;
    }
    const { descendantPid } = JSON.parse(result.stdout);
    assert.throws(() => process.kill(descendantPid, 0), { code: 'ESRCH' });
  });
}

for (const [signal, expectedCode] of [['SIGINT', 130], ['SIGTERM', 143], ['SIGHUP', 129]]) {
  test(`supervisor drains its tree when it receives ${signal}`, async () => {
    const child = spawn(supervisor, ['--grace-ms', '100', '--', process.execPath, fixture, 'wait'], { stdio: ['ignore', 'pipe', 'inherit'] });
    const exited = new Promise((resolve, reject) => {
      child.once('exit', resolve);
      child.once('error', reject);
    });
    const watchdog = setTimeout(() => child.kill('SIGKILL'), 4000);
    try {
      const descriptor = await new Promise((resolve, reject) => {
        child.stdout.once('data', data => resolve(JSON.parse(data.toString())));
        child.once('error', reject);
        child.once('exit', () => reject(new Error('fixture exited before readiness')));
      });
      child.kill(signal);
      assert.equal(await exited, expectedCode);
      assert.throws(() => process.kill(descriptor.descendantPid, 0), { code: 'ESRCH' });
    } finally {
      clearTimeout(watchdog);
      child.kill('SIGTERM');
      await exited;
    }
  });
}
