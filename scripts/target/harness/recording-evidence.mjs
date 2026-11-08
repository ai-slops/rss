import { createHash } from "node:crypto"
import { readdir, readFile } from "node:fs/promises"
import { join } from "node:path"
import { analyzeRecordingGroup } from "./media-analysis.mjs"
import { poll } from "./processes.mjs"

export async function waitForRawAndClosed(recordingsRoot, rawMinimum, closedMinimum, timeoutMs) {
  let observed = { raw: [], closed: [] }
  await poll(async () => {
    observed = await recordingFiles(recordingsRoot)
    return observed.raw.length >= rawMinimum && observed.closed.length >= closedMinimum
  }, timeoutMs, `${rawMinimum} raw and ${closedMinimum} closed recording fragments`)
  return observed
}

export async function recordingFiles(root) {
  const raw = []
  const catalogs = []
  await walk(root, (path) => {
    if (path.endsWith(".mkv")) raw.push(path)
    if (path.endsWith("/catalog.json")) catalogs.push(path)
  }).catch((error) => {
    if (error.code !== "ENOENT") throw error
  })
  const closed = []
  for (const path of catalogs) {
    const entries = JSON.parse(await readFile(path, "utf8"))
    for (const entry of entries) if (entry.decoded === true) closed.push(entry.path)
  }
  return { raw: raw.sort(), closed: [...new Set(closed)].sort(), catalogs: catalogs.sort() }
}

export async function hashFiles(paths) {
  const output = {}
  for (const path of paths) output[path] = createHash("sha256").update(await readFile(path)).digest("hex")
  return output
}

export function assertHashesUnchanged(before, after) {
  const names = Object.keys(before).sort()
  if (JSON.stringify(names) !== JSON.stringify(Object.keys(after).sort())) {
    throw new Error("Raw recording fragment set changed during recovery")
  }
  for (const path of names) if (before[path] !== after[path]) throw new Error(`Raw fragment was rewritten: ${path}`)
}

export async function analyzeSnapshotRecordings(snapshot, declarations, supervisorBinary) {
  const outputs = new Map(snapshot.project.outputs.map((output) => [output.id, output]))
  const sources = new Map(snapshot.project.sources.map((source) => [source.id, source]))
  const evidence = []
  for (const output of outputs.values()) {
    const recordings = snapshot.recordings.filter((recording) => recording.outputId === output.id)
    let expectation
    if (output.adapter.kind === "sourceRecording") {
      const source = sources.get(output.adapter.sourceId)
      const declaration = declarations.find((candidate) => candidate.name === source?.name)
      if (declaration === undefined) throw new Error(`Missing clean-source declaration for output ${output.id}`)
      expectation = {
        label: declaration.name,
        profile: { width: 1280, height: 720 },
        content: { kind: "clean", label: declaration.name, color: declaration.color, toneHz: declaration.toneHz },
      }
    } else if (output.adapter.kind === "programRecording") {
      expectation = {
        label: "Program",
        profile: { width: snapshot.project.canvas.width, height: snapshot.project.canvas.height },
        content: { kind: "program", toneHz: [440, 880] },
      }
    } else {
      continue
    }
    evidence.push({ output, recordings, analyzed: await analyzeRecordingGroup(recordings, expectation, supervisorBinary) })
  }
  return evidence
}

async function walk(root, visit) {
  for (const entry of await readdir(root, { withFileTypes: true })) {
    const path = join(root, entry.name)
    if (entry.isDirectory()) await walk(path, visit)
    else if (entry.isFile()) visit(path)
  }
}
