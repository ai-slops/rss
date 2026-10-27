import { spawn } from 'node:child_process';
import { copyFile, open, unlink } from 'node:fs/promises';
import net from 'node:net';

if (process.env.LIVE_PRODUCTION_OWNER_PID === undefined) throw new Error('Native lifecycle fixtures require an owning supervisor');
const [mode, state] = process.argv.slice(2);
const root = new URL('../../', import.meta.url).pathname.replace(/\/$/, '');
const port = await new Promise((resolve, reject) => {
  const server = net.createServer();
  server.once('error', reject);
  server.listen(0, '127.0.0.1', () => { const address = server.address(); server.close(() => resolve(address.port)); });
});
let executable = `${root}/target/debug/live-production`;
if (mode === 'deleted') {
  executable = `${state}/live-production`;
  await copyFile(`${root}/target/debug/live-production`, executable);
}
const log = await open(`${state}/server.log`, 'a', 0o600);
const app = spawn(executable, [], {
  cwd: root,
  stdio: ['ignore', log.fd, log.fd],
  env: { ...process.env, LIVE_PRODUCTION_BIND: `127.0.0.1:${port}`, LIVE_PRODUCTION_STATE_DIR: state,
    LIVE_PRODUCTION_PASSWORD: 'lifecycle-fixture-password', LIVE_PRODUCTION_SECURE_COOKIE: 'false' },
});
await log.close();
app.once('error', error => { throw error; });
app.once('exit', code => process.exit(code ?? 1));
const deadline = Date.now() + 10000;
for (;;) {
  try {
    const response = await fetch(`http://127.0.0.1:${port}/api/v1/snapshot`, { signal: AbortSignal.timeout(500) });
    if (response.status === 401) break;
  } catch (error) {
    if (!(error instanceof Error)) throw error;
  }
  if (Date.now() >= deadline) throw new Error('Native fixture did not become ready');
  await new Promise(resolve => setTimeout(resolve, 25));
}
if (mode === 'deleted') await unlink(executable);
console.info(JSON.stringify({ appPid: app.pid, deletedExecutable: mode === 'deleted' }));
if (mode === 'success' || mode === 'deleted') process.exit(0);
if (mode === 'failure') process.exit(37);
setInterval(() => {}, 1000);
