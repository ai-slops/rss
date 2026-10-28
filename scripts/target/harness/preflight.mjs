import { readdir, readFile } from "node:fs/promises"
import { spawnSync } from "node:child_process"
import { basename, join, resolve } from "node:path"
import { writePrivateJson } from "./private-files.mjs"

const REQUIRED_GST = [
  "compositor", "audiomixer", "splitmuxsink", "x264enc", "voaacenc",
  "srtsrc", "rtmpsink", "matroskamux", "mpegtsmux", "h264parse", "aacparse",
  "avdec_h264", "avdec_aac", "webrtcsink", "nicesrc",
]

export async function runPreflight({ pluginDirectory, reportPath, serverBinary, supervisorBinary, obsBinary }) {
  const checks = []
  const osRelease = parseOsRelease(await readFile("/etc/os-release", "utf8"))
  checks.push(check("ubuntu-os", osRelease.ID === "ubuntu", `${osRelease.PRETTY_NAME ?? osRelease.ID}`))
  const libc = command("ldd", ["--version"])
  checks.push(check("gnu-libc", libc.ok && /glibc|gnu libc/iu.test(libc.output) && !/musl/iu.test(libc.output), firstLine(libc.output)))
  const contamination = Object.entries(process.env)
    .filter(([name, value]) => value !== undefined && /^(LD_LIBRARY_PATH|LD_PRELOAD|GST_PLUGIN_PATH|PKG_CONFIG_SYSROOT_DIR|LIVE_PRODUCTION_SYSROOT)$/u.test(name) && /\.tools\/sysroot|alpine|musl/iu.test(value))
    .map(([name]) => name)
  checks.push(check("native-environment", contamination.length === 0, contamination.length === 0 ? "no Alpine sysroot variables" : contamination.join(",")))

  const tools = {
    obs: version(obsBinary, ["--version"]),
    libobs: version("pkg-config", ["--modversion", "libobs"]),
    gstreamer: version("gst-launch-1.0", ["--version"]),
    ffmpeg: version("ffmpeg", ["-version"]),
    rustc: version("rustc", ["--version"]),
    cargo: version("cargo", ["--version"]),
    node: version("node", ["--version"]),
    npm: version("npm", ["--version"]),
    compiler: version("cc", ["--version"]),
    pkgConfig: version("pkg-config", ["--version"]),
  }
  for (const [name, observed] of Object.entries(tools)) checks.push(check(`tool-${name}`, observed.available, observed.version))

  const gstPlugins = Object.fromEntries(REQUIRED_GST.map((name) => [name, command("gst-inspect-1.0", [name]).ok]))
  checks.push(check("gstreamer-elements", Object.values(gstPlugins).every(Boolean), Object.entries(gstPlugins).filter(([, present]) => !present).map(([name]) => name).join(",") || "all present"))
  const encoders = command("ffmpeg", ["-hide_banner", "-encoders"])
  const decoders = command("ffmpeg", ["-hide_banner", "-decoders"])
  checks.push(check("ffmpeg-h264-encoder", encoders.ok && /\b(libx264|h264_[a-z0-9_]+)\b/iu.test(encoders.output), "H.264 encoder capability"))
  checks.push(check("ffmpeg-aac-encoder", encoders.ok && /\baac\b/iu.test(encoders.output), "AAC encoder capability"))
  checks.push(check("ffmpeg-decoders", decoders.ok && /\bh264\b/iu.test(decoders.output) && /\baac\b/iu.test(decoders.output), "H.264/AAC decoders"))

  const nativeFiles = [resolve(serverBinary), resolve(supervisorBinary)]
  for (const path of nativeFiles) checks.push(nativeBinaryCheck(path))
  const obsPath = executablePath(obsBinary)
  if (obsPath !== null) {
    const obsNative = nativeBinaryCheck(obsPath)
    checks.push(obsNative)
    const obsLdd = command("ldd", [obsPath])
    checks.push(check("obs-links-libobs", obsLdd.ok && /libobs/iu.test(obsLdd.output), firstLine(obsLdd.output)))
  }
  const pluginFiles = await sharedObjects(pluginDirectory).catch(() => [])
  checks.push(check("obs-plugin-stage", pluginFiles.length > 0, `${pluginFiles.length} shared objects`))
  for (const path of pluginFiles) checks.push(nativeBinaryCheck(path))

  const report = {
    schemaVersion: 1,
    status: checks.every((entry) => entry.pass) ? "pass" : "fail",
    targetPolicy: "native Ubuntu GNU libc; capability checked; no Alpine sysroot",
    expected: {
      rust: "repo rust-version 1.91",
      developmentGStreamer: "1.26.9 measured Alpine development pin; target reports actual",
      developmentFfmpeg: "8.0.1 measured Alpine development pin; target reports actual",
      obs: "not pinned by repository; actual native version required",
    },
    actual: { osRelease, tools, gstPlugins },
    checks,
  }
  await writePrivateJson(reportPath, report)
  return report
}

function command(executable, args) {
  const result = spawnSync(executable, args, { encoding: "utf8", timeout: 15_000 })
  return { ok: result.status === 0, output: `${result.stdout ?? ""}${result.stderr ?? ""}`.trim() }
}

function version(executable, args) {
  const result = command(executable, args)
  return { available: result.ok, version: firstLine(result.output) || "unavailable" }
}

function nativeBinaryCheck(path) {
  const file = command("file", [path])
  const ldd = command("ldd", [path])
  const native = file.ok && ldd.ok && !/musl|alpine|\.tools\/sysroot/iu.test(`${file.output}\n${ldd.output}`)
  return check(`native-${basename(path)}`, native, native ? "GNU/Linux dependencies resolved" : `${firstLine(file.output)}; ${firstLine(ldd.output)}`)
}

function executablePath(name) {
  if (name.includes("/")) return resolve(name)
  const result = spawnSync("sh", ["-c", "command -v \"$1\"", "sh", name], { encoding: "utf8" })
  return result.status === 0 ? result.stdout.trim() : null
}

async function sharedObjects(directory) {
  const output = []
  async function walk(path) {
    for (const entry of await readdir(path, { withFileTypes: true })) {
      const child = join(path, entry.name)
      if (entry.isDirectory()) await walk(child)
      else if (entry.isFile() && entry.name.endsWith(".so")) output.push(child)
    }
  }
  await walk(directory)
  return output.sort()
}

function parseOsRelease(text) {
  return Object.fromEntries(text.split("\n").filter(Boolean).map((line) => {
    const [key, ...parts] = line.split("=")
    return [key, parts.join("=").replace(/^"|"$/gu, "")]
  }))
}

function check(name, pass, detail) {
  return { name, pass, detail }
}

function firstLine(value) {
  return value.split("\n", 1)[0] ?? ""
}
