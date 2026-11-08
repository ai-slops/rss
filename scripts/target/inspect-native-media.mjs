import { readdir, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { runSupervised } from "./harness/media-analysis.mjs"

const [supervisor, output, ...directories] = process.argv.slice(2)
if (supervisor === undefined || output === undefined || directories.length === 0) throw new Error("Expected supervisor, new output JSON and recording directories")
const evidence = []
for (const directory of directories) await walk(directory)
await writeFile(output, `${JSON.stringify({ schemaVersion: 1, measurements: evidence }, null, 2)}\n`, { mode: 0o600, flag: "wx" })

async function walk(directory) {
  for (const item of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, item.name)
    if (item.isDirectory()) await walk(path)
    else if (/^segment-\d+\.mkv$/u.test(item.name)) {
      try { inspect(path) }
      catch (error) { evidence.push({ path, status: "failed", detail: error.message }) }
    }
  }
}

function inspect(path) {
  const pcm = runSupervised(supervisor, "ffmpeg", ["-v", "error", "-i", path, "-map", "0:a:0", "-t", "2", "-ac", "1", "-ar", "48000", "-f", "f32le", "pipe:1"], { binary: true })
  const samples = Array.from({ length: Math.floor(pcm.length / 4) }, (_, index) => pcm.readFloatLE(index * 4))
  const power = {}
  for (const hz of [440, 660, 880]) {
    let real = 0; let imaginary = 0
    for (let index = 0; index < samples.length; index++) {
      const angle = 2 * Math.PI * hz * index / 48000
      real += samples[index] * Math.cos(angle); imaginary += samples[index] * Math.sin(angle)
    }
    power[hz] = samples.length === 0 ? 0 : (real * real + imaginary * imaginary) / samples.length ** 2
  }
  const video = runSupervised(supervisor, "ffmpeg", ["-v", "error", "-i", path, "-frames:v", "1", "-vf", "scale=64:36", "-pix_fmt", "rgb24", "-f", "rawvideo", "pipe:1"], { binary: true })
  const rgb = [0, 0, 0]
  for (let index = 0; index < video.length; index++) rgb[index % 3] += video[index]
  evidence.push({ path, samples: samples.length, rms: Math.sqrt(samples.reduce((sum, value) => sum + value * value, 0) / Math.max(samples.length, 1)),
    normalizedDftPower: power, firstFrameRgb: rgb.map(sum => sum / Math.max(video.length / 3, 1)) })
}
