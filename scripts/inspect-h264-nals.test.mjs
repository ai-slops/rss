import assert from 'node:assert/strict';
import test from 'node:test';
import { inspectNalPackets } from './inspect-h264-nals.mjs';

function packet(bytes, pts) {
  const groups = Buffer.from(bytes).toString('hex').match(/.{1,4}/gu) ?? [];
  return { pts_time: pts, data: `\n00000000: ${groups.join(' ').padEnd(39)}  test\n` };
}

test('counts SPS PPS and IDR with mixed Annex-B prefixes across packet boundaries', () => {
  const result = inspectNalPackets({ packets: [
    packet([0, 0, 0, 1, 0x67, 0x42, 0], '0.0'),
    packet([0, 1, 0x68, 0xaa, 0, 0, 0, 1, 0x65, 0xbb], '1.0'),
  ] });
  assert.deepEqual(result.parameterSets, { sps: 1, pps: 1, idr: 1 });
  assert.deepEqual(result.units.map(value => value.type), [7, 8, 5]);
  assert.deepEqual(result.units.map(value => value.size), [2, 2, 2]);
  assert.deepEqual(result.units.map(value => value.offset), [0, 6, 11]);
});

test('reports missing parameter sets without inventing them for slice packets', () => {
  const result = inspectNalPackets({ packets: [packet([0, 0, 1, 0x61, 0, 0, 1, 0x65], '2.0')] });
  assert.deepEqual(result.parameterSets, { sps: 0, pps: 0, idr: 1 });
  assert.equal(result.byteCount, 8);
});

test('rejects absent or malformed packet data instead of returning a clean inventory', () => {
  assert.throws(() => inspectNalPackets({}));
  assert.throws(() => inspectNalPackets({ packets: [{}] }));
  assert.throws(() => inspectNalPackets({ packets: [{ data: 'not a hex dump' }] }));
});
