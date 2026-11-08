import assert from "node:assert/strict"
import test from "node:test"
import { createHash } from "node:crypto"
import { mkdtemp, readFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { analyzeRecordingGroup, runSupervised } from "./media-analysis.mjs"

const supervisor = process.env.LIVE_PRODUCTION_TEST_SUPERVISOR
const enabled = supervisor !== undefined

test("decodes exact clean and Program profiles with content evidence", { skip: !enabled }, async () => {
  const directory = await mkdtemp(join(tmpdir(), "lp-media-analysis-"))
  const red = join(directory, "red.mkv")
  const blue = join(directory, "blue.mkv")
  const program = join(directory, "program.mkv")
  makeClean(red, "red", 440)
  makeClean(blue, "blue", 880)
  makeProgram(program)
  const redEvidence = await analyzeRecordingGroup([await recording(red)], {
    label: "red", profile: { width: 1280, height: 720 },
    content: { kind: "clean", label: "red", color: "red", toneHz: 440 },
  }, supervisor)
  const blueEvidence = await analyzeRecordingGroup([await recording(blue)], {
    label: "blue", profile: { width: 1280, height: 720 },
    content: { kind: "clean", label: "blue", color: "blue", toneHz: 880 },
  }, supervisor)
  const programEvidence = await analyzeRecordingGroup([await recording(program)], {
    label: "Program", profile: { width: 960, height: 540 },
    content: { kind: "program", toneHz: [440, 880] },
  }, supervisor)
  assert.equal(redEvidence.content.color, "red")
  assert.equal(blueEvidence.content.color, "blue")
  assert.deepEqual(programEvidence.content.tonesHz, [440, 880])
  await assert.rejects(analyzeRecordingGroup([await recording(red)], {
    label: "wrong-profile", profile: { width: 960, height: 540 },
    content: { kind: "clean", label: "red", color: "red", toneHz: 440 },
  }, supervisor), /unexpected dimensions/u)
})

function makeClean(path, color, toneHz) {
  runSupervised(supervisor, "ffmpeg", [
    "-v", "error", "-y",
    "-f", "lavfi", "-i", `color=c=${color}:s=1280x720:r=30:d=1`,
    "-f", "lavfi", "-i", "testsrc2=s=160x90:r=30:d=1",
    "-f", "lavfi", "-i", `sine=frequency=${toneHz}:sample_rate=48000:duration=1`,
    "-filter_complex", "[0:v][1:v]overlay=x=20:y=H-h-20[v]",
    "-map", "[v]", "-map", "2:a", "-c:v", "libx264", "-pix_fmt", "yuv420p",
    "-c:a", "aac", "-ar", "48000", "-ac", "2", "-shortest", path,
  ], { timeoutMs: 60_000 })
}

function makeProgram(path) {
  runSupervised(supervisor, "ffmpeg", [
    "-v", "error", "-y",
    "-f", "lavfi", "-i", "color=c=red:s=480x540:r=30:d=1",
    "-f", "lavfi", "-i", "color=c=blue:s=480x540:r=30:d=1",
    "-f", "lavfi", "-i", "sine=frequency=440:sample_rate=48000:duration=1",
    "-f", "lavfi", "-i", "sine=frequency=880:sample_rate=48000:duration=1",
    "-filter_complex", "[0:v][1:v]hstack=inputs=2[v];[2:a][3:a]amix=inputs=2:normalize=0[a]",
    "-map", "[v]", "-map", "[a]", "-c:v", "libx264", "-pix_fmt", "yuv420p",
    "-c:a", "aac", "-ar", "48000", "-ac", "2", "-shortest", path,
  ], { timeoutMs: 60_000 })
}

async function recording(path) {
  const sha256 = createHash("sha256").update(await readFile(path)).digest("hex")
  return { path, sha256, validation: "decoded" }
}
