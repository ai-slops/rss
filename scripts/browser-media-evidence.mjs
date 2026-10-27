import { expect } from '../web/node_modules/playwright/test.mjs';

export async function waitForPrivateAudio(page, timeout = 20000) {
  await expect.poll(() => page.evaluate(async () => {
    const audio = document.querySelector('audio');
    if (audio === null || !(audio.srcObject instanceof MediaStream) || audio.muted || audio.paused || audio.readyState < 3 || audio.currentTime <= 0) return false;
    const track = audio.srcObject.getAudioTracks()[0];
    if (track === undefined) return false;
    const peer = (globalThis.__liveProductionPeerConnections ?? []).find(peer => peer.getReceivers().some(receiver => receiver.track.id === track.id));
    if (peer === undefined || peer.connectionState !== 'connected') return false;
    const stats = await peer.getStats();
    return [...stats.values()].some(report => report.type === 'inbound-rtp' && report.kind === 'audio' && report.packetsReceived > 3 && report.bytesReceived > 100);
  }), { timeout }).toBe(true);
}

export async function peerStats(page) {
  return page.evaluate(async () => {
    const audio = document.querySelector('audio');
    const privateTrack = audio?.srcObject instanceof MediaStream ? audio.srcObject.getAudioTracks()[0]?.id : undefined;
    const reports = [];
    for (const peer of globalThis.__liveProductionPeerConnections ?? []) {
      const inbound = [];
      for (const report of (await peer.getStats()).values()) {
        if (report.type === 'inbound-rtp') inbound.push({ kind: report.kind, bytesReceived: report.bytesReceived, packetsReceived: report.packetsReceived, framesDecoded: report.framesDecoded, jitter: report.jitter });
      }
      reports.push({ connectionState: peer.connectionState, iceConnectionState: peer.iceConnectionState,
        privateSource: privateTrack !== undefined && peer.getReceivers().some(receiver => receiver.track.id === privateTrack), inbound });
    }
    return reports;
  });
}

export async function videoColors(page) {
  return page.locator('video').evaluateAll(videos => videos.map(video => {
    const canvas = document.createElement('canvas'); canvas.width = 16; canvas.height = 9;
    const context = canvas.getContext('2d');
    if (context === null || video.readyState < 2) return { stream: video.getAttribute('aria-label'), range: 0 };
    context.drawImage(video, 0, 0, 16, 9);
    const samples = [...context.getImageData(0, 1, 16, 1).data].filter((_, index) => index % 4 !== 3);
    return { stream: video.getAttribute('aria-label'), range: Math.max(...samples) - Math.min(...samples) };
  }));
}
