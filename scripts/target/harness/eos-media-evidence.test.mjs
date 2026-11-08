import assert from "node:assert/strict"
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import test from "node:test"
import { summarizeEosLog } from "./eos-media-evidence.mjs"

test("EOS timeline preserves branch and fragment observations without treating any bus EOS as recorder proof", async context => {
  const directory = await mkdtemp(join(tmpdir(), "eos-evidence-"))
  context.after(() => rm(directory, { recursive: true, force: true }))
  await writeFile(join(directory, "test.log"), [
    "0:03:00.000 GST_BUS posting message eos from sender-pipeline",
    "0:03:00.010 splitmuxsink Got Reference EOS. Finishing up",
    "0:03:00.011 splitmuxsink Have EOS event at pad <queue_audio_0:src>",
    "0:03:00.012 splitmuxsink Sending splitmuxsink-fragment-closed message",
    "drift_stop_return stop_call_elapsed_ms=15100 result=Err(PipelineDetail(\"recording pipeline did not finalize with EOS\"))",
  ].join("\n"))
  const counts = await summarizeEosLog(directory)
  assert.equal(counts.busEosCandidates, 1)
  assert.equal(counts.queueEos, 1)
  assert.equal(counts.fragmentClosed, 1)
  assert.equal(counts.errorLines, 1)
  const report = JSON.parse(await readFile(join(directory, "events.json"), "utf8"))
  assert.deepEqual(report.events.map(value => value.line), [1, 2, 3, 4, 5])
  assert.equal(Object.hasOwn(report, "accepted"), false)
})
