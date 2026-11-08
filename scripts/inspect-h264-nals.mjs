import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

export function inspectNalPackets(document) {
  if (!Array.isArray(document.packets)) throw new Error('Expected ffprobe packets JSON');
  let byteOffset = 0;
  const packetRanges = document.packets.map((packet, index) => {
    if (typeof packet.data !== 'string') throw new Error(`Packet ${index} has no data dump`);
    const hex = packet.data.split('\n').filter(line => line.length > 0).map(line => {
      if (!/^[0-9a-f]{8}: /iu.test(line)) throw new Error('Unexpected ffprobe data row');
      const value = line.slice(10, 49).replaceAll(' ', '');
      if (!/^[0-9a-f]*$/iu.test(value) || value.length % 2 !== 0) throw new Error('Invalid ffprobe hex data');
      return value;
    }).join('');
    const bytes = Buffer.from(hex, 'hex');
    const start = byteOffset;
    byteOffset += bytes.length;
    return { index, start, end: byteOffset, ptsTime: packet.pts_time ?? null, bytes };
  });
  const bytes = Buffer.concat(packetRanges.map(packet => packet.bytes));
  const starts = [];
  for (let index = 0; index + 3 < bytes.length; index++) {
    if (bytes[index] !== 0 || bytes[index + 1] !== 0) continue;
    const prefix = bytes[index + 2] === 1 ? 3
      : bytes[index + 2] === 0 && bytes[index + 3] === 1 ? 4 : 0;
    if (prefix === 0 || index + prefix >= bytes.length) continue;
    starts.push({ offset: index, prefix, type: bytes[index + prefix] & 31 });
    index += prefix - 1;
  }
  const counts = {};
  let packetIndex = 0;
  const units = starts.map((unit, index) => {
    counts[unit.type] = (counts[unit.type] ?? 0) + 1;
    while (packetIndex + 1 < packetRanges.length && unit.offset >= packetRanges[packetIndex].end) packetIndex++;
    return {
      ...unit,
      size: (starts[index + 1]?.offset ?? bytes.length) - unit.offset - unit.prefix,
      packetIndex,
      ptsTime: packetRanges[packetIndex]?.ptsTime ?? null,
    };
  });
  return { schemaVersion: 1, structuralOnly: true, packetCount: packetRanges.length, byteCount: bytes.length,
    parameterSets: { sps: counts[7] ?? 0, pps: counts[8] ?? 0, idr: counts[5] ?? 0 }, counts, units };
}

if (process.argv[1] !== undefined && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) {
  if (process.argv[2] === '--help') {
    console.log('Usage: node scripts/inspect-h264-nals.mjs FFPROBE_PACKET_DATA_JSON\nInventory Annex-B NAL headers only; this does not validate decoding or infer provenance.');
  } else {
    if (process.argv.length !== 3) throw new Error('Expected one ffprobe -show_packets -show_data JSON file');
    console.log(JSON.stringify(inspectNalPackets(JSON.parse(await readFile(process.argv[2], 'utf8'))), null, 2));
  }
}
