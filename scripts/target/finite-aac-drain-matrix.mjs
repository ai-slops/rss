import { mkdir, stat, readFile, writeFile } from "node:fs/promises"
import { join, resolve } from "node:path"
import { createHash, randomBytes } from "node:crypto"
import { setTimeout as delay } from "node:timers/promises"
import { ensureSupervised } from "../runner-supervision.mjs"
import { OwnedProcesses, freeUdpPort, poll } from "./harness/processes.mjs"
import { runSupervised } from "./harness/media-analysis.mjs"

await ensureSupervised()
const root = resolve(new URL("../..", import.meta.url).pathname)
const [supervisor, directory] = process.argv.slice(2)
if (!supervisor || !directory) throw new Error("Expected supervisor and fresh private evidence root")
await mkdir(directory, { mode: 0o700 })
const results = []
for (const load of [false, true]) {
  for (const profile of [{ drainNs: 0, receiverDelayMs: 0 }, { drainNs: 0, receiverDelayMs: 1000 }, { drainNs: 500000000, receiverDelayMs: 0 }]) {
    results.push(await execute(load, profile))
    await writeFile(join(directory, "matrix.json"), `${JSON.stringify({ schemaVersion: 1,
      claim: "native-gstreamer-eos-ownership-not-obs", diagnosticOnly: true, cases: results }, null, 2)}\n`, { mode: 0o600 })
  }
}
process.exitCode = results.some(value => !value.source.strictDecoded || value.profile.drainNs > 0
  && (!value.receiver.strictDecoded || Number(value.receiver.frames.find(stream => stream.kind === "video")?.frames) !== 150)) ? 1 : 0

async function execute(load, profile) {
  const label = `${load ? "loaded" : "idle"}-sender-${profile.drainNs}-receiver-${profile.receiverDelayMs}`
  const output = join(directory, label)
  await mkdir(output, { mode: 0o700 })
  const processes = new OwnedProcesses(supervisor, { timeoutMs: 20000, graceMs: 3000 })
  const port = await freeUdpPort()
  const passphrase = randomBytes(24).toString("hex")
  const receiverPath = join(output, "receiver.ts"); const sourcePath = join(output, "source.ts")
  try {
    const receiver = await processes.spawn("drain-receiver", "gst-launch-1.0", ["-q", "-e", "srtsrc",
      `uri=srt://127.0.0.1:${port}?mode=listener&passphrase=${passphrase}&pbkeylen=16`,
      "latency=125", "keep-listening=false", "!", "filesink", "buffer-mode=unbuffered", `location=${receiverPath}`],
    { cwd: root, env: process.env, logPath: join(output, "receiver.log") })
    await poll(async () => (await readFile(`/proc/${receiver.nativePid}/net/udp`, "utf8"))
      .includes(`:${port.toString(16).toUpperCase().padStart(4, "0")}`), 3000, "owned SRT listener bind")
    const loads = []
    if (load) for (let index = 0; index < 2; index++) loads.push(await processes.spawn(`cpu-${index}`, "gst-launch-1.0", [
      "-q", "videotestsrc", "num-buffers=5000", "!", "video/x-raw,width=1280,height=720,framerate=30/1,format=I420", "!",
      "x264enc", "threads=1", "tune=zerolatency", "speed-preset=ultrafast", "!", "fakesink", "sync=false"],
    { cwd: root, env: process.env, logPath: join(output, `load-${index}.log`) }))
    const start = Date.now()
    const sender = await processes.spawn("drain-sender", "gst-launch-1.0", ["-q", "-e", "mpegtsmux", "name=mux", "alignment=7",
      "!", "tee", "name=wire", "wire.", "!", "queue", "!", "filesink", "buffer-mode=unbuffered", `location=${sourcePath}`,
      "wire.", "!", "queue", "!", "srtsink", `uri=srt://127.0.0.1:${port}?mode=caller&passphrase=${passphrase}&pbkeylen=16`,
      "latency=125", "wait-for-connection=true", "auto-reconnect=false",
      "wire.", "!", "queue", "!", "fakesink", "sync=true", `ts-offset=${profile.drainNs}`,
      "videotestsrc", "is-live=true", "num-buffers=150", "pattern=ball", "!", "video/x-raw,width=640,height=360,framerate=30/1,format=I420",
      "!", "x264enc", "threads=2", "tune=zerolatency", "key-int-max=30", "bitrate=1000", "byte-stream=true", "option-string=repeat-headers=1",
      "!", "h264parse", "config-interval=-1", "!", "mux.",
      "audiotestsrc", "is-live=true", "num-buffers=235", "wave=sine", "freq=440", "!", "audio/x-raw,rate=48000,channels=2",
      "!", "voaacenc", "bitrate=128000", "!", "aacparse", "!", "mux."],
    { cwd: root, env: process.env, logPath: join(output, "sender.log") })
    await processes.wait(sender, 15000)
    await delay(profile.receiverDelayMs)
    await processes.wait(receiver, 15000)
    for (const worker of loads) await processes.stop(worker)
    return { label, profile, load, durationMs: Date.now() - start,
      senderExit: sender.supervisor.exitCode, receiverExit: receiver.supervisor.exitCode,
      source: await inspect(sourcePath, output, "source"), receiver: await inspect(receiverPath, output, "receiver") }
  } finally { await processes.stopAll() }
}

async function inspect(path, output, name) {
  const probe = JSON.parse(runSupervised(supervisor, "ffprobe", ["-v", "error", "-count_frames", "-show_streams", "-show_format", "-of", "json", path]))
  await writeFile(join(output, `${name}.ffprobe.json`), JSON.stringify(probe, null, 2), { mode: 0o600 })
  let strictDecoded = true; let error = null
  try { runSupervised(supervisor, "ffmpeg", ["-v", "error", "-xerror", "-i", path, "-f", "null", "-"]) }
  catch (failure) { strictDecoded = false; error = failure.message }
  await writeFile(join(output, `${name}.decode.json`), JSON.stringify({ strictDecoded, error }), { mode: 0o600 })
  return { bytes: (await stat(path)).size, sha256: createHash("sha256").update(await readFile(path)).digest("hex"), strictDecoded,
    frames: probe.streams.map(stream => ({ kind: stream.codec_type, codec: stream.codec_name, frames: stream.nb_read_frames })) }
}
