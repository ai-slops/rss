import { createHash } from "node:crypto"
import { readFile } from "node:fs/promises"
import { spawnSync } from "node:child_process"

export async function analyzeRecordingGroup(recordings, expectation, supervisorBinary) {
  if (recordings.length === 0) throw new Error(`Output ${expectation.label} has no recording segments`)
  const segments = []
  for (const recording of recordings) segments.push(await inspectSegment(recording, expectation.profile, supervisorBinary))
  const paths = recordings.map((recording) => recording.path)
  const content = expectation.content.kind === "clean"
    ? analyzeCleanContent(paths, expectation.content, supervisorBinary)
    : analyzeProgramContent(paths, expectation.content, supervisorBinary)
  return { label: expectation.label, segments, content }
}

export function runSupervised(supervisorBinary, executable, args, options = {}) {
  const result = spawnSync(supervisorBinary, [
    "--timeout-ms", `${options.timeoutMs ?? 60_000}`, "--grace-ms", "5000", "--",
    executable, ...args,
  ], {
    encoding: options.binary ? null : "utf8",
    maxBuffer: options.maxBuffer ?? 128 * 1024 * 1024,
    timeout: (options.timeoutMs ?? 60_000) + 10_000,
  })
  if (result.status !== 0) throw new Error(`${executable} failed: ${result.stderr?.toString("utf8") ?? "unknown"}`)
  return result.stdout
}

async function inspectSegment(recording, profile, supervisorBinary) {
  const bytes = await readFile(recording.path)
  const probe = JSON.parse(runSupervised(supervisorBinary, "ffprobe", [
    "-v", "error", "-show_streams", "-show_format", "-of", "json", recording.path,
  ]))
  runSupervised(supervisorBinary, "ffmpeg", ["-v", "error", "-xerror", "-i", recording.path, "-f", "null", "-"])
  const videoStreams = (probe.streams ?? []).filter((stream) => stream.codec_type === "video")
  const audioStreams = (probe.streams ?? []).filter((stream) => stream.codec_type === "audio")
  if (videoStreams.length !== 1 || audioStreams.length !== 1) throw new Error(`${recording.path} must contain exactly one video and one audio stream`)
  const video = videoStreams[0]; const audio = audioStreams[0]
  if (video.codec_name !== "h264" || audio.codec_name !== "aac") throw new Error(`${recording.path} is not H.264/AAC`)
  if (video.width !== profile.width || video.height !== profile.height) throw new Error(`${recording.path} has unexpected dimensions`)
  if (Number.parseInt(audio.sample_rate, 10) !== 48_000 || audio.channels !== 2) throw new Error(`${recording.path} is not stereo 48 kHz`)
  const sha256 = createHash("sha256").update(bytes).digest("hex")
  if (recording.sha256 !== sha256 || recording.validation !== "decoded") throw new Error(`${recording.path} catalog validation/checksum differs`)
  return {
    sha256,
    bytes: bytes.length,
    durationSeconds: Number.parseFloat(probe.format?.duration ?? "0"),
    video: { codec: video.codec_name, width: video.width, height: video.height },
    audio: { codec: audio.codec_name, sampleRate: 48_000, channels: 2 },
  }
}

function analyzeCleanContent(paths, content, supervisorBinary) {
  const frames = decodeFrames(paths, supervisorBinary)
  const averages = frames.map(channelAverage)
  const average = averages.reduce((best, candidate) => colorRatio(candidate, content.color) > colorRatio(best, content.color) ? candidate : best)
  if (content.color === "red" && average.red <= average.blue * 1.5) throw new Error(`${content.label} did not decode red`)
  if (content.color === "blue" && average.blue <= average.red * 1.5) throw new Error(`${content.label} did not decode blue`)
  const motion = maximumFrameDifference(frames)
  if (motion <= 0.05) throw new Error(`${content.label} frame counter did not change`)
  const powers = strongestTone(paths, content.toneHz, supervisorBinary)
  assertDominantTone(powers, content.toneHz, content.label)
  return { kind: "clean", color: content.color, frameCounterDifference: motion, toneHz: content.toneHz, powers }
}

function analyzeProgramContent(paths, content, supervisorBinary) {
  const frames = decodeFrames(paths, supervisorBinary)
  const layouts = frames.map((frame) => ({ left: halfAverage(frame, false), right: halfAverage(frame, true) }))
  const layout = layouts.find(({ left, right }) => left.red > left.blue * 1.5 && right.blue > right.red * 1.5)
  if (layout === undefined) throw new Error("Program did not decode red-left and blue-right composition")
  const { left, right } = layout
  if (left.red <= left.blue * 1.5 || right.blue <= right.red * 1.5) throw new Error("Program did not decode red-left and blue-right composition")
  const powers = strongestProgramTones(paths, content.toneHz, supervisorBinary)
  for (const toneHz of content.toneHz) {
    if (powers[toneHz] <= powers.noise * 4) {
      throw new Error(`Program omitted routed ${toneHz} Hz tone: ${JSON.stringify(powers)}`)
    }
  }
  return { kind: "program", layout: "red-left-blue-right", tonesHz: [...content.toneHz], powers }
}

function decodeFrames(paths, supervisorBinary) {
  const frameBytes = 64 * 36 * 3
  const frames = []
  for (const path of paths) {
    const raw = runSupervised(supervisorBinary, "ffmpeg", [
      "-v", "error", "-i", path, "-vf", "fps=2,scale=64:36", "-frames:v", "4",
      "-pix_fmt", "rgb24", "-f", "rawvideo", "pipe:1",
    ], { binary: true })
    for (let offset = 0; offset + frameBytes <= raw.length; offset += frameBytes) frames.push(raw.subarray(offset, offset + frameBytes))
  }
  if (frames.length < 2) throw new Error("Recording group yielded fewer than two decoded frames")
  return frames
}

function tonePowers(path, supervisorBinary) {
  const raw = runSupervised(supervisorBinary, "ffmpeg", [
    "-v", "error", "-i", path, "-map", "0:a:0", "-t", "2", "-ac", "1", "-ar", "48000",
    "-f", "f32le", "pipe:1",
  ], { binary: true })
  const samples = new Float32Array(Math.floor(raw.length / 4))
  for (let index = 0; index < samples.length; index += 1) samples[index] = raw.readFloatLE(index * 4)
  return { 440: goertzel(samples, 440), 880: goertzel(samples, 880), noise: goertzel(samples, 660) }
}

function strongestTone(paths, expectedHz, supervisorBinary) {
  return paths
    .map((path) => tonePowers(path, supervisorBinary))
    .reduce((best, candidate) => toneRatio(candidate, expectedHz) > toneRatio(best, expectedHz) ? candidate : best)
}

function strongestProgramTones(paths, tones, supervisorBinary) {
  return paths
    .map((path) => tonePowers(path, supervisorBinary))
    .reduce((best, candidate) => programToneRatio(candidate, tones) > programToneRatio(best, tones) ? candidate : best)
}

function toneRatio(powers, expectedHz) {
  const alternate = expectedHz === 440 ? 880 : 440
  return powers[expectedHz] / Math.max(powers[alternate], powers.noise, Number.EPSILON)
}

function programToneRatio(powers, tones) {
  return Math.min(...tones.map((tone) => powers[tone] / Math.max(powers.noise, Number.EPSILON)))
}

function assertDominantTone(powers, expectedHz, label) {
  const alternate = expectedHz === 440 ? 880 : 440
  if (powers[expectedHz] <= Math.max(powers[alternate], powers.noise) * 4) throw new Error(`${label} omitted dominant ${expectedHz} Hz tone: ${JSON.stringify(powers)}`)
}

function goertzel(samples, frequency) {
  const coefficient = 2 * Math.cos((2 * Math.PI * frequency) / 48_000)
  let previous = 0; let previous2 = 0
  for (const sample of samples) { const current = sample + coefficient * previous - previous2; previous2 = previous; previous = current }
  return previous2 ** 2 + previous ** 2 - coefficient * previous * previous2
}

function channelAverage(frame) {
  let red = 0; let green = 0; let blue = 0
  for (let index = 0; index < frame.length; index += 3) { red += frame[index]; green += frame[index + 1]; blue += frame[index + 2] }
  const pixels = frame.length / 3
  return { red: red / pixels, green: green / pixels, blue: blue / pixels }
}

function colorRatio(average, color) {
  return color === "red"
    ? average.red / Math.max(average.blue, Number.EPSILON)
    : average.blue / Math.max(average.red, Number.EPSILON)
}

function halfAverage(frame, rightHalf) {
  let red = 0; let green = 0; let blue = 0; let pixels = 0
  for (let y = 0; y < 36; y += 1) {
    for (let x = rightHalf ? 32 : 0; x < (rightHalf ? 64 : 32); x += 1) {
      const index = (y * 64 + x) * 3
      red += frame[index]; green += frame[index + 1]; blue += frame[index + 2]; pixels += 1
    }
  }
  return { red: red / pixels, green: green / pixels, blue: blue / pixels }
}

function maximumFrameDifference(frames) {
  let maximum = 0
  for (let frame = 1; frame < frames.length; frame += 1) {
    let total = 0
    for (let index = 0; index < frames[frame].length; index += 1) total += Math.abs(frames[frame - 1][index] - frames[frame][index])
    maximum = Math.max(maximum, total / frames[frame].length)
  }
  return maximum
}
