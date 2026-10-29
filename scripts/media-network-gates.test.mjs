import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const script = await readFile(new URL('./media-network-gates.sh', import.meta.url), 'utf8');

test('SRT fixture owns in-band H264 parameter sets across GStreamer versions', () => {
  assert.match(script, /video\/x-raw,format=I420/u);
  assert.match(script, /x264enc[^\n]*byte-stream=true[^\n]*option-string=repeat-headers=1/u);
  assert.match(script, /video\/x-h264,stream-format=byte-stream,alignment=au/u);
  assert.match(script, /h264parse[^\n]*config-interval=-1[^\n]*disable-passthrough=true/u);
});

test('network gate rejects received video without H264 dimensions', () => {
  assert.match(script, /-select_streams v:0/u);
  assert.match(script, /stream=codec_name,width,height/u);
  assert.match(script, /h264,640,360/u);
  assert.match(script, /ffmpeg -v error -xerror/u);
});
