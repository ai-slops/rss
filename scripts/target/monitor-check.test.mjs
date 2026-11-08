import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

import { privateAudioEvidence, sanitizedFailure, summarizePeerReports, sharedStateUnchanged } from './support/monitor-report.mjs';

test('peer report summary omits candidate addresses and SDP-shaped fields', () => {
  const reports = [
    { id: 'local', type: 'local-candidate', candidateType: 'relay', protocol: 'udp', address: '10.0.0.2', port: 50000 },
    { id: 'remote', type: 'remote-candidate', candidateType: 'host', protocol: 'udp', address: '192.0.2.1', port: 40000 },
    { id: 'pair', type: 'candidate-pair', state: 'succeeded', nominated: true, localCandidateId: 'local', remoteCandidateId: 'remote' },
    { id: 'video', type: 'inbound-rtp', kind: 'video', framesDecoded: 40, packetsReceived: 100, bytesReceived: 2000, sdp: 'secret' },
  ];

  const summary = summarizePeerReports(reports);

  assert.deepEqual(summary.selectedPair, { localCandidateType: 'relay', remoteCandidateType: 'host', protocol: 'udp' });
  assert.equal(summary.inbound[0].framesDecoded, 40);
  assert.equal(JSON.stringify(summary).includes('10.0.0.2'), false);
  assert.equal(JSON.stringify(summary).includes('secret'), false);
});

test('browser-local monitoring proves shared program and routes did not change', () => {
  const before = { program: { programRevision: 'program-7' }, project: { audioRoutes: [{ trackId: 'a', gainDb: 0, muted: false, routed: true, inputPeakDb: -30, outputPeakDb: -40 }] } };
  const after = { program: { programRevision: 'program-7' }, project: { audioRoutes: [{ trackId: 'a', gainDb: 0, muted: false, routed: true, inputPeakDb: -4, outputPeakDb: -6 }] } };

  assert.equal(sharedStateUnchanged(before, after), true);
  assert.equal(sharedStateUnchanged(before, { ...after, program: { programRevision: 'program-8' } }), false);
  assert.equal(sharedStateUnchanged(before, { ...after, project: { audioRoutes: [{ ...after.project.audioRoutes[0], muted: true }] } }), false);
  assert.equal(sharedStateUnchanged(before, { ...after, project: { audioRoutes: [{ ...after.project.audioRoutes[0], gainDb: -3 }] } }), false);
});

test('private audio must belong to the peer receiving the audio element track', () => {
  const peers = [
    { receiverTracks: [{ id: 'private-track', kind: 'audio' }, { id: 'program-video', kind: 'video' }], inbound: [{ kind: 'audio', packetsReceived: 900 }] },
    { receiverTracks: [{ id: 'private-track', kind: 'audio' }], inbound: [{ kind: 'audio', packetsReceived: 0 }] },
  ];

  assert.deepEqual(privateAudioEvidence('private-track', peers, 1), {
    privatePeerMatched: false,
    matchedPeerIndex: 1,
    packetsReceived: 0,
    audioOnly: true,
    createdAfterListen: true,
  });
  assert.deepEqual(privateAudioEvidence('private-track', [peers[0], { ...peers[1], inbound: [{ kind: 'audio', packetsReceived: 4 }] }], 1), {
    privatePeerMatched: true,
    matchedPeerIndex: 1,
    packetsReceived: 4,
    audioOnly: true,
    createdAfterListen: true,
  });
});

test('program peer cannot satisfy private audio evidence', () => {
  const peers = [
    { receiverTracks: [{ id: 'private-track', kind: 'audio' }, { id: 'program-video', kind: 'video' }], inbound: [{ kind: 'audio', packetsReceived: 900 }] },
  ];

  assert.deepEqual(privateAudioEvidence('private-track', peers, 1), {
    privatePeerMatched: false,
    matchedPeerIndex: null,
    packetsReceived: 0,
    audioOnly: false,
    createdAfterListen: false,
  });
});

test('private packet wait does not use an async waitForFunction predicate', async () => {
  const source = await readFile(new URL('./monitor-check.mjs', import.meta.url), 'utf8');

  assert.doesNotMatch(source, /waitForFunction\(async/u);
});

test('monitoring failures are artifact-safe', () => {
  const error = new Error('locator Audio source Camera One at https://private.example.test failed');

  assert.equal(sanitizedFailure(error), 'monitoring_check_failed');
  assert.equal(sanitizedFailure(error).includes('Camera One'), false);
});
