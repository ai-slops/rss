import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

const url = new URL(process.env.LIVE_PRODUCTION_URL ?? 'http://127.0.0.1:3100/');
assert.ok(['http:', 'https:'].includes(url.protocol) && url.username === '' && url.password === '');
const password = (await readFile(new URL('../var/operator-password', import.meta.url), 'utf8')).trimEnd();
const api = new URL('/api/v1/', url);
const request = (path, options = {}) => fetch(new URL(path, api), { signal: AbortSignal.timeout(10_000), ...options });

assert.equal((await fetch(url, { signal: AbortSignal.timeout(10_000) })).status, 200);
assert.equal((await request('snapshot')).status, 401);
const login = await request('auth/login', {
  method: 'POST', headers: { 'content-type': 'application/json', 'x-live-production': '1' },
  body: JSON.stringify({ password }),
});
assert.equal(login.status, 204);
const setCookie = login.headers.get('set-cookie');
assert.ok(setCookie?.includes('HttpOnly') && setCookie.includes('SameSite=Strict'));
if (url.protocol === 'https:') assert.ok(setCookie.includes('Secure'));
const cookie = setCookie.split(';')[0];
assert.ok(cookie);
const authenticated = await request('snapshot', { headers: { cookie } });
assert.equal(authenticated.status, 200);
const snapshot = await authenticated.json();
assert.equal(typeof snapshot.engineRunId, 'string');
assert.ok(Array.isArray(snapshot.project.scenes));
assert.ok(snapshot.project.scenes.length >= 1);
if (process.env.LIVE_PRODUCTION_REQUIRE_STOPPED === '1') {
  assert.ok(snapshot.outputStatuses.every(output => output.desired === 'stopped' && ['stopped', 'failed'].includes(output.actual)),
    'Active or finalizing outputs block this development-service refresh');
}
const logout = await request('auth/logout', { method: 'POST', headers: { cookie, 'x-live-production': '1' } });
assert.equal(logout.status, 204);
assert.equal((await request('snapshot', { headers: { cookie } })).status, 401);
console.info('PASS: UI reachable; unauthenticated API denied; private login, snapshot and logout verified. No credentials printed.');
