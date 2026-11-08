import { mkdir, readFile } from "node:fs/promises"
import { join, resolve } from "node:path"
import { ensureSupervised } from "../runner-supervision.mjs"
import { OwnedProcesses } from "./harness/processes.mjs"
import { writePrivateJson } from "./harness/private-files.mjs"
import { inspectEosMedia, summarizeEosLog } from "./harness/eos-media-evidence.mjs"

await ensureSupervised()
const root = resolve(new URL("../..", import.meta.url).pathname)
const [supervisor, directory] = process.argv.slice(2)
if (!supervisor || !directory) throw new Error("Expected supervisor and a fresh private evidence root")
await mkdir(directory, { mode: 0o700 })
const plan = [
  { name: "idle-minimal", load: false, trace: false },
  { name: "idle-traced", load: false, trace: true },
  { name: "loaded-minimal", load: true, trace: false },
  { name: "loaded-traced", load: true, trace: true },
]
const cases = []
const save = () => writePrivateJson(join(directory, "matrix.json"), {
  schemaVersion: 1, diagnosticOnly: true, acceptanceClaim: false, fixedPlan: plan, cases })
await save()
for (const profile of plan) {
  const output = join(directory, profile.name)
  await mkdir(output, { mode: 0o700 })
  await mkdir(join(output, "tmp"), { mode: 0o700 })
  const result = { profile, startedAt: new Date().toISOString() }
  const processes = new OwnedProcesses(supervisor, { timeoutMs: 300000, graceMs: 10000 })
  try {
    const env = { ...process.env, LIVE_PRODUCTION_EOS_OBSERVE: "1",
      LIVE_PRODUCTION_MEDIA_EVIDENCE_DIR: join(output, "media"), TMPDIR: join(output, "tmp"), GST_DEBUG_NO_COLOR: "1" }
    delete env.GST_DEBUG_FILE
    if (profile.trace) env.GST_DEBUG = "splitmuxsink:5,matroskamux:4,GST_EVENT:5,GST_BUS:5,GST_STATES:4,queue:5"
    else delete env.GST_DEBUG
    if (profile.load) {
      const worker = await processes.spawn("one-cpu-load", process.execPath,
        ["--eval", "const end=Date.now()+310000;while(Date.now()<end){}"],
        { cwd: root, env: process.env, logPath: join(output, "load.log") })
      result.loadWorker = { pid: worker.nativePid,
        statAtStart: await readFile(`/proc/${worker.nativePid}/stat`, "utf8") }
    }
    const test = await processes.spawn("original-drift-soak", "cargo", ["test", "--locked", "-p",
      "live-production-media", "--test", "srt_drift_endurance", "--", "--exact",
      "sustained_independent_clocks_report_drift_and_preserve_bounded_progress", "--nocapture"],
    { cwd: root, env, logPath: join(output, "test.log") })
    result.testPid = test.nativePid
    await processes.wait(test, 330000)
    result.exitCode = test.supervisor.exitCode
    result.signal = test.supervisor.signalCode
  } catch (error) {
    result.harnessError = error.message
  } finally {
    await processes.stopAll()
    result.testFinishedAt = new Date().toISOString()
  }
  try {
    const media = await inspectEosMedia(output, supervisor)
    if (media.length === 0) throw new Error("No retained raw recording fragments were available for inspection")
    result.rawSegments = media.length
    result.rawStrictFailures = media.filter(item => !item.strictDecoded).length
    result.events = await summarizeEosLog(output)
  } catch (error) { result.inspectionError = error.message }
  result.finishedAt = new Date().toISOString()
  cases.push(result)
  await save()
  process.stdout.write(`${JSON.stringify({ name: profile.name, exitCode: result.exitCode,
    rawSegments: result.rawSegments, rawStrictFailures: result.rawStrictFailures,
    harnessError: result.harnessError, inspectionError: result.inspectionError })}\n`)
}
process.exitCode = cases.some(value => value.exitCode !== 0 || value.rawStrictFailures > 0
  || value.harnessError || value.inspectionError) ? 1 : 0
