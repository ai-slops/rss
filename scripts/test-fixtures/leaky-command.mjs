import { spawn } from 'node:child_process';

const mode = process.argv[2] ?? 'success';
if (mode === 'worker') {
  process.on('SIGTERM', () => {});
  process.send?.('ready');
  setInterval(() => {}, 1000);
} else {
  const child = spawn(process.execPath, [new URL(import.meta.url).pathname, 'worker'], {
    stdio: ['ignore', 'ignore', 'ignore', 'ipc'],
    detached: mode === 'escaped',
  });
  child.once('error', error => { throw error; });
  child.once('message', () => {
    console.info(JSON.stringify({ descendantPid: child.pid }));
    if (mode === 'success' || mode === 'escaped') process.exit(0);
    if (mode === 'failure') process.exit(23);
  });
  setInterval(() => {}, 1000);
}
