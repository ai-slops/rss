import { createHash } from "node:crypto"
import { createReadStream } from "node:fs"
import { access, readdir, readFile, stat } from "node:fs/promises"
import { join } from "node:path"
import { createInterface } from "node:readline"
import { runSupervised } from "./media-analysis.mjs"
import { writePrivateJson } from "./private-files.mjs"

export async function inspectEosMedia(directory, supervisor) {
  const retained = join(directory, "media/srt-drift-finalized-or-failed")
  const root = await access(retained).then(() => retained).catch(error => {
    if (error.code !== "ENOENT") throw error
    return join(directory, "tmp")
  })
  const measurements = []
  await walk(root)
  await writePrivateJson(join(directory, "raw-media.json"), { root, measurements,
    rawInspectionOnly: true, sessionAcceptanceUnchanged: true })
  return measurements

  async function walk(path) {
    for (const item of await readdir(path, { withFileTypes: true })) {
      const file = join(path, item.name)
      if (item.isDirectory()) await walk(file)
      else if (/^segment-\d+\.mkv$/u.test(item.name)) {
        const measurement = { path: file, bytes: (await stat(file)).size,
          sha256: createHash("sha256").update(await readFile(file)).digest("hex") }
        try {
          measurement.probe = JSON.parse(runSupervised(supervisor, "ffprobe", [
            "-v", "error", "-count_frames", "-show_streams", "-show_format", "-of", "json", file]))
          runSupervised(supervisor, "ffmpeg", ["-v", "error", "-xerror", "-i", file, "-f", "null", "-"])
          measurement.strictDecoded = true
        } catch (error) {
          measurement.strictDecoded = false
          measurement.detail = error.message
        }
        measurements.push(measurement)
      }
    }
  }
}

export async function summarizeEosLog(directory) {
  const events = []; const counts = { fragmentOpened: 0, fragmentClosed: 0, referenceEos: 0, queueEos: 0,
    asyncEos: 0, busEosCandidates: 0, errorLines: 0 }
  let line = 0
  const input = createInterface({ input: createReadStream(join(directory, "test.log")), crlfDelay: Infinity })
  for await (const text of input) {
    line++
    for (const [name, expression] of [
      ["fragmentOpened", /Sending splitmuxsink-fragment-opened/u],
      ["fragmentClosed", /Sending splitmuxsink-fragment-closed/u],
      ["referenceEos", /Got Reference EOS/u], ["queueEos", /Have EOS event at pad/u],
      ["asyncEos", /Sent async EOS/u], ["busEosCandidates", /(?:posting|pushing|popped).*\beos\b/iu],
      ["errorLines", /\bERROR\b|Error:|did not finalize with EOS/u],
    ]) if (expression.test(text)) counts[name]++
    if (/drift_stop|drift_stopped|drift_before_finalize|drift_evidence|Got Reference EOS|Have EOS event at pad|Sending splitmuxsink-fragment-(?:opened|closed)|Sent async EOS|Error:|\bERROR\b|did not finalize with EOS|test result:/u.test(text)
      || /GST_BUS.*\beos\b/iu.test(text)) {
      events.push({ line, text })
    }
  }
  const result = { counts, events, totalLogLines: line,
    limitation: "Bus EOS candidates include every pipeline; category counts alone do not establish recorder EOS." }
  await writePrivateJson(join(directory, "events.json"), result)
  return counts
}
