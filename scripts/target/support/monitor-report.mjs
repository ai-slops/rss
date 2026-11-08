export function summarizePeerReports(reports) {
  const byId = new Map(reports.map(report => [report.id, report]));
  const selectedPairId = reports.find(report => report.type === 'transport' && typeof report.selectedCandidatePairId === 'string')?.selectedCandidatePairId;
  const selectedPair = selectedPairId === undefined ? undefined : byId.get(selectedPairId);
  const pair = selectedPair ?? reports.find(report => report.type === 'candidate-pair' && report.state === 'succeeded' && report.nominated === true);
  const local = pair === undefined ? undefined : byId.get(pair.localCandidateId);
  const remote = pair === undefined ? undefined : byId.get(pair.remoteCandidateId);
  return {
    selectedPair: pair === undefined ? null : {
      localCandidateType: candidateType(local),
      remoteCandidateType: candidateType(remote),
      protocol: protocol(local, pair),
    },
    localCandidateTypes: uniqueCandidateTypes(reports, 'local-candidate'),
    remoteCandidateTypes: uniqueCandidateTypes(reports, 'remote-candidate'),
    inbound: reports.filter(report => report.type === 'inbound-rtp').map(report => ({
      kind: typeof report.kind === 'string' ? report.kind : 'unknown',
      bytesReceived: finiteNumber(report.bytesReceived),
      packetsReceived: finiteNumber(report.packetsReceived),
      framesDecoded: finiteNumber(report.framesDecoded),
      jitter: finiteNumber(report.jitter),
    })),
  };
}

export function sharedStateUnchanged(before, after) {
  return before?.program?.programRevision === after?.program?.programRevision
    && JSON.stringify(sharedAudioRoutes(before)) === JSON.stringify(sharedAudioRoutes(after));
}

export function privateAudioEvidence(audioTrackId, peers, firstPeerIndex) {
  const relativeIndex = peers.slice(firstPeerIndex).findIndex(candidate => candidate.receiverTracks.some(track => track.id === audioTrackId && track.kind === 'audio')
    && candidate.receiverTracks.length > 0
    && candidate.receiverTracks.every(track => track.kind === 'audio'));
  if (relativeIndex < 0) return emptyPrivateAudioEvidence();
  const matchedPeerIndex = firstPeerIndex + relativeIndex;
  const packetsReceived = Math.max(0, ...peers[matchedPeerIndex].inbound
    .filter(report => report.kind === 'audio')
    .map(report => finiteNumber(report.packetsReceived) ?? 0));
  return {
    privatePeerMatched: packetsReceived > 3,
    matchedPeerIndex,
    packetsReceived,
    audioOnly: true,
    createdAfterListen: matchedPeerIndex >= firstPeerIndex,
  };
}

export function sanitizedFailure(_error) {
  return 'monitoring_check_failed';
}

function candidateType(report) {
  return typeof report?.candidateType === 'string' ? report.candidateType : 'unknown';
}

function protocol(candidate, pair) {
  if (typeof candidate?.protocol === 'string') return candidate.protocol;
  return typeof pair?.protocol === 'string' ? pair.protocol : 'unknown';
}

function uniqueCandidateTypes(reports, type) {
  return [...new Set(reports.filter(report => report.type === type).map(candidateType))].sort();
}

function finiteNumber(value) {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

function emptyPrivateAudioEvidence() {
  return {
    privatePeerMatched: false,
    matchedPeerIndex: null,
    packetsReceived: 0,
    audioOnly: false,
    createdAfterListen: false,
  };
}

function sharedAudioRoutes(snapshot) {
  return (snapshot?.project?.audioRoutes ?? []).map(route => ({
    trackId: route.trackId,
    gainDb: route.gainDb,
    muted: route.muted,
    routed: route.routed,
  })).sort((left, right) => String(left.trackId).localeCompare(String(right.trackId)));
}
