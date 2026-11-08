import { mkdir, readFile, stat, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { chromium } from '../../web/node_modules/playwright/index.mjs';

import { privateAudioEvidence, sanitizedFailure, sharedStateUnchanged, summarizePeerReports } from './support/monitor-report.mjs';

const baseUrl = requiredUrl(process.env.LIVE_PRODUCTION_URL);
const passwordFile = requiredPath(process.env.LIVE_PRODUCTION_PASSWORD_FILE, 'LIVE_PRODUCTION_PASSWORD_FILE');
const reportPath = resolve(process.env.LIVE_PRODUCTION_MONITOR_REPORT ?? 'var/target-monitor-report.json');
const timeout = Number.parseInt(process.env.LIVE_PRODUCTION_MONITOR_TIMEOUT_MS ?? '30000', 10);
const password = await privatePassword(passwordFile);
const launch = process.env.LIVE_PRODUCTION_CHROME === undefined
  ? { channel: 'chrome', headless: process.env.LIVE_PRODUCTION_HEADLESS === 'true' }
  : { executablePath: resolve(process.env.LIVE_PRODUCTION_CHROME), headless: process.env.LIVE_PRODUCTION_HEADLESS === 'true' };

const browser = await chromium.launch(launch);
let report;
try {
  const context = await browser.newContext();
  const page = await context.newPage();
  await page.addInitScript(() => {
    const NativePeerConnection = globalThis.RTCPeerConnection;
    globalThis.__targetMonitorPeers = [];
    globalThis.RTCPeerConnection = class extends NativePeerConnection {
      constructor(configuration) {
        super(configuration);
        globalThis.__targetMonitorPeers.push(this);
      }
    };
  });

  await page.goto(baseUrl, { waitUntil: 'domcontentloaded', timeout });
  const passwordInput = page.getByLabel('Operator password');
  if (await passwordInput.count() === 1) {
    await passwordInput.fill(password);
    await page.getByRole('button', { name: 'Open control room' }).click();
  }
  await page.getByRole('main').waitFor({ timeout });
  const controlBefore = await authenticatedJson(page, '/api/v1/snapshot');
  const capability = await authenticatedJson(page, '/api/v1/media/network-capabilities');
  const initialMedia = await mediaState(page);
  const initiallyMuted = initialMedia.every(media => media.muted);
  const videoResult = await waitForChangingVideo(page, initialMedia, timeout);
  const privateAudioAttempt = await enablePrivateListen(page, videoResult.length, timeout);
  const controlAfter = await authenticatedJson(page, '/api/v1/snapshot');
  const peerReports = await page.evaluate(async () => {
    const result = [];
    for (const peer of globalThis.__targetMonitorPeers ?? []) {
      result.push({
        connectionState: peer.connectionState,
        iceConnectionState: peer.iceConnectionState,
        receiverTracks: peer.getReceivers().flatMap(receiver => receiver.track === null ? [] : [{ id: receiver.track.id, kind: receiver.track.kind }]),
        stats: [...(await peer.getStats()).values()],
      });
    }
    return result;
  });
  report = {
    checkedAt: new Date().toISOString(),
    control: { authenticated: true, sharedStateUnchanged: sharedStateUnchanged(controlBefore, controlAfter) },
    capabilities: capability,
    media: {
      initiallyMuted,
      videos: videoResult,
      privateAudio: privateAudioResult(privateAudioAttempt, peerReports),
    },
    ice: peerReports.map((peer, peerIndex) => ({
      peerIndex,
      receiverKinds: [...new Set(peer.receiverTracks.map(track => track.kind))].sort(),
      createdAfterListen: peerIndex >= privateAudioAttempt.firstPeerIndex,
      connectionState: peer.connectionState,
      iceConnectionState: peer.iceConnectionState,
      ...summarizePeerReports(peer.stats),
    })),
  };
} catch (error) {
  report = {
    checkedAt: new Date().toISOString(),
    control: { authenticated: false, sharedStateUnchanged: false },
    media: { initiallyMuted: null, videos: [], privateAudio: { result: 'not_checked' } },
    ice: [],
    error: sanitizedFailure(error),
  };
} finally {
  await browser.close();
}

await mkdir(dirname(reportPath), { recursive: true, mode: 0o700 });
await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, { mode: 0o600 });
process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
if (!passed(report)) process.exitCode = 1;

async function privatePassword(path) {
  const metadata = await stat(path);
  if (!metadata.isFile() || metadata.size > 4096 || (metadata.mode & 0o077) !== 0) {
    throw new Error('Operator password file must be a private regular file');
  }
  const value = (await readFile(path, 'utf8')).trimEnd();
  if (value.length < 16) throw new Error('Operator password file does not contain a valid credential');
  return value;
}

async function authenticatedJson(page, path) {
  return page.evaluate(async endpoint => {
    const response = await fetch(endpoint, { credentials: 'same-origin' });
    if (!response.ok) throw new Error(`Authenticated request failed with HTTP ${response.status}`);
    return response.json();
  }, path);
}

async function mediaState(page) {
  return page.locator('video').evaluateAll(videos => videos.map(video => ({
    stream: video.getAttribute('aria-label'),
    frames: video.getVideoPlaybackQuality().totalVideoFrames,
    readyState: video.readyState,
    muted: video.muted,
    width: video.videoWidth,
    height: video.videoHeight,
  })));
}

async function waitForChangingVideo(page, initial, timeoutMs) {
  await page.waitForFunction(start => [...document.querySelectorAll('video')].every(video => {
    const label = video.getAttribute('aria-label');
    const previous = start.find(candidate => candidate.stream === label)?.frames ?? 0;
    return video.readyState >= 2 && video.getVideoPlaybackQuality().totalVideoFrames >= previous + 5;
  }), initial, { timeout: timeoutMs }).catch(() => undefined);
  const current = await mediaState(page);
  return current.map(media => ({
    ...media,
    changing: media.frames >= (initial.find(candidate => candidate.stream === media.stream)?.frames ?? 0) + 5,
  }));
}

async function enablePrivateListen(page, baselinePeerCount, timeoutMs) {
  await page.getByRole('button', { name: 'setup' }).click();
  const audioRoutes = page.getByRole('button', { name: 'Audio routes' });
  if (await audioRoutes.count() === 1) await audioRoutes.click();
  const listen = page.getByRole('button', { name: /Listen before mix|Listening privately/ }).first();
  if (await listen.count() === 0) return { result: 'source_audio_unavailable', trackId: null, firstPeerIndex: baselinePeerCount };
  await page.waitForFunction(expected => (globalThis.__targetMonitorPeers ?? []).length >= expected, baselinePeerCount, { timeout: timeoutMs });
  const firstPeerIndex = await page.evaluate(() => (globalThis.__targetMonitorPeers ?? []).length);
  await listen.click();
  const enable = page.getByRole('button', { name: 'Enable browser monitoring' });
  if (await enable.count() === 1) await enable.click();
  const matched = await page.waitForFunction(peerFloor => {
    const audio = document.querySelector('audio');
    if (!(audio?.srcObject instanceof MediaStream) || audio.muted || audio.paused) return false;
    const track = audio.srcObject.getAudioTracks()[0];
    if (track === undefined) return false;
    const peer = (globalThis.__targetMonitorPeers ?? []).slice(peerFloor).find(candidate => {
      const receivers = candidate.getReceivers().flatMap(receiver => receiver.track === null ? [] : [receiver.track]);
      return receivers.some(receiver => receiver.id === track.id && receiver.kind === 'audio') && receivers.every(receiver => receiver.kind === 'audio');
    });
    return peer !== undefined;
  }, firstPeerIndex, { timeout: timeoutMs }).then(() => true).catch(() => false);
  const trackId = await page.evaluate(() => {
    const audio = document.querySelector('audio');
    return audio?.srcObject instanceof MediaStream ? audio.srcObject.getAudioTracks()[0]?.id ?? null : null;
  });
  const received = matched && trackId !== null && await waitForPrivatePackets(page, firstPeerIndex, trackId, timeoutMs);
  return { result: received ? 'received' : 'no_decoded_audio', trackId, firstPeerIndex };
}

async function waitForPrivatePackets(page, firstPeerIndex, trackId, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const received = await page.evaluate(async ({ peerFloor, expectedTrackId }) => {
      const peer = (globalThis.__targetMonitorPeers ?? []).slice(peerFloor).find(candidate => {
        const receivers = candidate.getReceivers().flatMap(receiver => receiver.track === null ? [] : [receiver.track]);
        return receivers.some(receiver => receiver.id === expectedTrackId && receiver.kind === 'audio')
          && receivers.length > 0
          && receivers.every(receiver => receiver.kind === 'audio');
      });
      if (peer === undefined) return false;
      return [...(await peer.getStats()).values()].some(value => value.type === 'inbound-rtp'
        && value.kind === 'audio'
        && value.packetsReceived > 3);
    }, { peerFloor: firstPeerIndex, expectedTrackId: trackId });
    if (received) return true;
    await new Promise(resolveDelay => setTimeout(resolveDelay, 100));
  }
  return false;
}

function privateAudioResult(attempt, peerReports) {
  const peerCountObserved = peerReports.length;
  if (attempt.trackId === null) return {
    result: attempt.result,
    privatePeerMatched: false,
    matchedPeerIndex: null,
    packetsReceived: 0,
    audioOnly: false,
    createdAfterListen: false,
    peerCountBeforeListen: attempt.firstPeerIndex,
    peerCountObserved,
  };
  const peers = peerReports.map(peer => ({
    receiverTracks: peer.receiverTracks,
    inbound: peer.stats.filter(report => report.type === 'inbound-rtp').map(report => ({ kind: report.kind, packetsReceived: report.packetsReceived })),
  }));
  const evidence = privateAudioEvidence(attempt.trackId, peers, attempt.firstPeerIndex);
  return {
    ...evidence,
    result: evidence.privatePeerMatched ? 'received' : 'no_decoded_audio',
    peerCountBeforeListen: attempt.firstPeerIndex,
    peerCountObserved,
  };
}

function passed(value) {
  return value.control?.authenticated === true
    && value.control.sharedStateUnchanged === true
    && value.media?.initiallyMuted === true
    && value.media.videos.length >= 2
    && value.media.videos.every(video => video.changing)
    && value.media.privateAudio?.result === 'received'
    && value.media.privateAudio.privatePeerMatched === true
    && value.media.privateAudio.audioOnly === true
    && value.media.privateAudio.createdAfterListen === true
    && value.media.privateAudio.packetsReceived > 3
    && value.ice.some(peer => peer.peerIndex === value.media.privateAudio.matchedPeerIndex
      && peer.createdAfterListen === true
      && peer.receiverKinds.length === 1
      && peer.receiverKinds[0] === 'audio'
      && peer.inbound.some(report => report.kind === 'audio' && report.packetsReceived > 3))
    && value.ice.some(peer => peer.connectionState === 'connected');
}

function requiredUrl(value) {
  if (value === undefined) throw new Error('LIVE_PRODUCTION_URL is required');
  const url = new URL(value);
  const loopback = url.hostname === '127.0.0.1' || url.hostname === 'localhost';
  if (url.protocol !== 'https:' && !(url.protocol === 'http:' && loopback)) {
    throw new Error('Monitoring requires HTTPS except on loopback');
  }
  return url.toString();
}

function requiredPath(value, name) {
  if (value === undefined) throw new Error(`${name} is required`);
  return resolve(value);
}
