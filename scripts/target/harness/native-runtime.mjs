import { access, mkdir } from "node:fs/promises"
import { join } from "node:path"
import { ApiClient } from "./api-client.mjs"
import { freeTcpPort, poll } from "./processes.mjs"

export async function startServer(processes, options, privateDirectory, password, phase) {
  const port = await freeTcpPort()
  const stateDirectory = join(privateDirectory, "state")
  await mkdir(stateDirectory, { recursive: true, mode: 0o700 })
  return startServerAt(processes, options, privateDirectory, password, phase, stateDirectory, port)
}

export async function startServerAt(processes, options, privateDirectory, password, phase, stateDirectory, port) {
  const record = await processes.spawn(`server-${phase}`, options.serverBinary, [], {
    cwd: options.root,
    logPath: join(privateDirectory, `${phase}-server.log`),
    env: {
      ...process.env,
      LIVE_PRODUCTION_BIND: `127.0.0.1:${port}`,
      LIVE_PRODUCTION_STATE_DIR: stateDirectory,
      LIVE_PRODUCTION_WEB_DIST: options.webDist,
      LIVE_PRODUCTION_PASSWORD: password,
      LIVE_PRODUCTION_SECURE_COOKIE: "false",
    },
  })
  const origin = `http://127.0.0.1:${port}`
  await poll(async () => fetch(origin, { signal: AbortSignal.timeout(500) }).then(() => true).catch(() => false), 30_000, `${phase} server HTTP`)
  const api = new ApiClient(`${origin}/api/v1`)
  await api.login(password)
  return { record, api, origin, stateDirectory, port }
}

export async function startVirtualDisplay(processes, options, privateDirectory) {
  if (process.env.DISPLAY !== undefined && process.env.DISPLAY.length > 0) {
    return { record: null, display: process.env.DISPLAY }
  }
  const display = await availableDisplay()
  const record = await processes.spawn("xvfb", options.xvfbBinary, [display, "-screen", "0", "1280x720x24", "-nolisten", "tcp"], {
    cwd: options.root,
    logPath: join(privateDirectory, "xvfb.log"),
    env: process.env,
  })
  await poll(() => access(`/tmp/.X11-unix/X${display.slice(1)}`).then(() => true).catch(() => false), 10_000, `Xvfb ${display}`)
  return { record, display }
}

export async function startObs(processes, options, privateDirectory, fixtureFile, display) {
  return startObsWithFixture(processes, options, privateDirectory, display,
    { LIVE_PRODUCTION_OBS_FIXTURE_FILE: fixtureFile })
}

export async function startRecoveryObs(processes, options, privateDirectory, fixtureFile, display) {
  return startObsWithFixture(processes, options, privateDirectory, display,
    { LIVE_PRODUCTION_OBS_RECOVERY_FIXTURE_FILE: fixtureFile })
}

async function startObsWithFixture(processes, options, privateDirectory, display, fixtureEnvironment) {
  const config = join(privateDirectory, "obs-config")
  const data = join(privateDirectory, "obs-data")
  await mkdir(config, { recursive: true, mode: 0o700 })
  await mkdir(data, { recursive: true, mode: 0o700 })
  const pluginBin = join(options.pluginDirectory, "bin/64bit")
  const pluginData = join(options.pluginDirectory, "data/obs-plugins")
  return processes.spawn("obs", options.obsBinary, [
    "--multi", "--disable-shutdown-check", "--collection", "Live Production Rehearsal",
    "--profile", "Live Production Rehearsal", "--scene", "Live Production Rehearsal",
  ], {
    cwd: options.root,
    logPath: join(privateDirectory, "obs.log"),
    env: {
      ...process.env,
      DISPLAY: display,
      XDG_CONFIG_HOME: config,
      XDG_DATA_HOME: data,
      OBS_PLUGINS_PATH: pluginBin,
      OBS_PLUGINS_DATA_PATH: pluginData,
      ...fixtureEnvironment,
    },
  })
}

async function availableDisplay() {
  for (let number = 90; number <= 120; number += 1) {
    const path = `/tmp/.X11-unix/X${number}`
    if (await access(path).then(() => false).catch(() => true)) return `:${number}`
  }
  throw new Error("No private X11 display number is available")
}
