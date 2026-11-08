import { watch } from "node:fs"
import { readFile } from "node:fs/promises"
import { dirname, join } from "node:path"
import { createHash } from "node:crypto"
import { startServer } from "./native-runtime.mjs"
import { assertHashesUnchanged, hashFiles, recordingFiles, waitForRawAndClosed } from "./recording-evidence.mjs"
import { poll } from "./processes.mjs"

export async function proveAbruptRecovery(context) {
  const { api, processes, server, options, privateDirectory, password, stateDirectory } = context
  let snapshot = await api.snapshot()
  await api.command("output.create", snapshot.project.id, {
    name: "Abrupt Program",
    adapter: { kind: "programRecording", segmentSeconds: 1 },
  }, { snapshot })
  snapshot = await api.snapshot()
  const output = snapshot.project.outputs.find((candidate) => candidate.name === "Abrupt Program")
  if (output === undefined) throw new Error("Abrupt recovery output was not created")
  await api.command("output.start", output.id, {}, { snapshot })
  const recordingRoot = join(stateDirectory, "recordings", output.id)
  const active = await waitForRawAndClosed(recordingRoot, 1, 0, 30_000)
  const sessionDirectory = dirname(active.raw[0])
  const tailPath = await stopOnFragmentCreation({
    directory: sessionDirectory,
    fragmentName: "segment-00003.mkv",
    timeoutMs: 10_000,
    stop: () => processes.signalNative(server.record, "SIGSTOP"),
  })
  await waitForStopped(server.record.nativePid)
  const beforeFiles = await recordingFiles(recordingRoot)
  if (beforeFiles.raw.length < 4 || beforeFiles.closed.length < 2) {
    throw new Error("Abrupt fixture did not produce four raw and two closed fragments")
  }
  if (beforeFiles.closed.includes(tailPath)) throw new Error("Abrupt tail closed before SIGSTOP")
  const beforeHashes = await hashFiles(beforeFiles.raw)
  processes.signalNative(server.record, "SIGKILL")
  await processes.wait(server.record, 2_000)
  const afterKill = await recordingFiles(recordingRoot)
  assertHashesUnchanged(beforeHashes, await hashFiles(afterKill.raw))

  const restarted = await startServer(processes, options, privateDirectory, password, "restarted")
  const recoveredSnapshot = await restarted.api.snapshot()
  const status = recoveredSnapshot.outputStatuses.find((candidate) => candidate.outputId === output.id)
  if (status?.desired !== "stopped" || status.actual !== "stopped") {
    throw new Error("Restarted output was not stopped")
  }
  const recovered = recoveredSnapshot.recordings.filter((recording) => recording.outputId === output.id)
  if (recovered.length !== beforeFiles.raw.length) throw new Error("Restart inventory differs from raw fragments")
  if (recovered.filter((recording) => recording.validation === "decoded").length < 2) {
    throw new Error("Fewer than two closed fragments survived abrupt recovery")
  }
  const tail = recovered.find((recording) => recording.path === tailPath)
  if (tail?.recoveryStatus !== "partial" || tail.validation !== "partial") {
    throw new Error("Interrupted tail is not visible as partial")
  }
  if (!recovered.every((recording) => recording.gaps.includes("abrupt_shutdown"))) {
    throw new Error("Recovered fragments omitted abrupt_shutdown gaps")
  }
  const afterRestart = await recordingFiles(recordingRoot)
  assertHashesUnchanged(beforeHashes, await hashFiles(afterRestart.raw))
  await verifyCatalogHashes(recovered)
  return {
    restarted,
    outputId: output.id,
    rawFragments: recovered.length,
    confirmedClosedBeforeKill: beforeFiles.closed.length,
    decodedAfterRestart: recovered.filter((recording) => recording.validation === "decoded").length,
    recoveries: [...new Set(recovered.map((recording) => recording.recoveryStatus))].sort(),
    hashesPreserved: true,
  }
}

async function verifyCatalogHashes(recordings) {
  for (const recording of recordings) {
    const digest = createHash("sha256").update(await readFile(recording.path)).digest("hex")
    if (digest !== recording.sha256) throw new Error(`Recovered checksum mismatch: ${recording.path}`)
  }
}

async function waitForStopped(pid) {
  await poll(async () => {
    const status = await readFile(`/proc/${pid}/status`, "utf8").catch(() => "")
    return /^State:\s+T/mu.test(status)
  }, 5_000, `server ${pid} to enter SIGSTOP`)
}

export function stopOnFragmentCreation({ directory, fragmentName, timeoutMs, stop }) {
  const path = join(directory, fragmentName)
  return new Promise((resolve, reject) => {
    const watcher = watch(directory, { persistent: false }, (_event, filename) => {
      if (filename?.toString() !== fragmentName) return
      clearTimeout(timer)
      watcher.close()
      try {
        stop(path)
        resolve(path)
      } catch (error) {
        reject(error)
      }
    })
    const timer = setTimeout(() => {
      watcher.close()
      reject(new Error(`Timed out waiting for recording fragment ${path}`))
    }, timeoutMs)
    watcher.once("error", (error) => {
      clearTimeout(timer)
      reject(error)
    })
  })
}
