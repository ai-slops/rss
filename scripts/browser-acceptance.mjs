import { mkdir, writeFile } from "node:fs/promises"
import { chromium } from "../web/node_modules/playwright/index.mjs"
import { ensureSupervised } from './runner-supervision.mjs'
import { peerStats, videoColors, waitForPrivateAudio } from './browser-media-evidence.mjs'
import { exerciseOperatorControls } from './browser-operator-evidence.mjs'
await ensureSupervised()

const baseUrl = process.env.LIVE_PRODUCTION_URL ?? "http://127.0.0.1:3001"
const password = process.env.LIVE_PRODUCTION_PASSWORD ?? "test-operator-password"
const evidenceDirectory = new URL(`${process.env.BROWSER_EVIDENCE_DIR ?? "../.tools/browser-evidence"}/`, import.meta.url)
const browserRoot = new URL("../.tools/browser-root/", import.meta.url).pathname
await mkdir(evidenceDirectory, { recursive: true })

const browser = await chromium.launch({
  executablePath: new URL("./browser-chromium.sh", import.meta.url).pathname,
  env: {
    ...process.env,
    LD_LIBRARY_PATH: `${browserRoot}usr/lib/pulseaudio:${browserRoot}usr/lib:${browserRoot}lib`,
    FONTCONFIG_PATH: `${browserRoot}etc/fonts`,
    XDG_DATA_DIRS: `${browserRoot}usr/share`,
  },
  headless: true,
  args: ["--no-sandbox", "--disable-dev-shm-usage"],
})
let page
try {
const context = await browser.newContext({ viewport: { width: 1280, height: 900 } })
page = await context.newPage()
const browserErrors = []
const commandResponses = []
page.on("console", (message) => {
  if (message.type() === "error") browserErrors.push(message.text())
})
page.on("pageerror", (error) => browserErrors.push(error.message))
page.on("response", async (response) => {
  if (!response.url().endsWith("/api/v1/commands")) return
  commandResponses.push({ status: response.status(), body: await response.json() })
})
await page.addInitScript(() => {
  const NativePeerConnection = globalThis.RTCPeerConnection
  globalThis.__liveProductionPeerConnections = []
  globalThis.RTCPeerConnection = class extends NativePeerConnection {
    constructor(configuration) {
      super(configuration)
      globalThis.__liveProductionPeerConnections.push(this)
    }
  }
})

await page.goto(baseUrl)
await page.getByLabel("Operator password").fill(password)
await page.getByRole("button", { name: "Open control room" }).click()
await page.getByRole("main").filter({ has: page.getByText("Untitled production") }).waitFor()
const initialSnapshot = await page.evaluate(async () => (await fetch("/api/v1/snapshot")).json())
await page.getByRole("button", { name: "setup" }).click()
await page.getByLabel("Input type").selectOption("synthetic")
await page.getByLabel("Source name").fill("Synthetic bars")
await page.getByRole("button", { name: "Create input", exact: true }).click()
await page.getByRole("article").filter({ has: page.getByText("Synthetic bars", { exact: true }) }).waitFor({ timeout: 15_000 })
await page.getByRole("button", { name: "Add video to Preview", exact: true }).click()
await page.getByRole("tab", { name: "Layers" }).click()
await page.getByText("Synthetic bars").last().waitFor({ timeout: 15_000 })
const operatorControls = await exerciseOperatorControls(page)
try {
  await page.getByText(/decoded frames/).first().waitFor({ timeout: 30_000 })
} catch (error) {
  const diagnostics = {
    browserErrors,
    monitorText: await page.locator(".monitor").allTextContents(),
    peerStates: await page.evaluate(() => (globalThis.__liveProductionPeerConnections ?? []).map((peerConnection) => ({
      connectionState: peerConnection.connectionState,
      iceConnectionState: peerConnection.iceConnectionState,
      signalingState: peerConnection.signalingState,
    }))),
  }
  await writeFile(new URL("connection-failure.json", evidenceDirectory), `${JSON.stringify(diagnostics, null, 2)}\n`)
  await page.screenshot({ path: new URL("connection-failure.png", evidenceDirectory).pathname, fullPage: true })
  throw error
}
try {
  await page.waitForFunction(() => {
    const preview = document.querySelector('video[aria-label="preview server-rendered media"]')
    return preview !== null && preview.getVideoPlaybackQuality().totalVideoFrames > 10
  }, null, { timeout: 30_000 })
} catch (error) {
  const diagnostics = {
    browserErrors,
    commandResponses,
    notices: await page.locator(".notice").allTextContents(),
    snapshot: await page.evaluate(async () => (await fetch("/api/v1/snapshot")).json()),
    media: await page.locator("video").evaluateAll((videos) => videos.map((video) => ({
      label: video.getAttribute("aria-label"),
      readyState: video.readyState,
      width: video.videoWidth,
      height: video.videoHeight,
      frames: video.getVideoPlaybackQuality().totalVideoFrames,
    }))),
  }
  await writeFile(new URL("preview-progress-failure.json", evidenceDirectory), `${JSON.stringify(diagnostics, null, 2)}\n`)
  await page.screenshot({ path: new URL("preview-progress-failure.png", evidenceDirectory).pathname, fullPage: true })
  throw error
}
const beforeTakeColors = await videoColors(page)
if (!beforeTakeColors.some(video => video.stream?.startsWith("preview") && video.range > 90)) throw new Error("Preview did not display the selected synthetic source pixels")
await page.getByRole("button", { name: "TAKE EXACT REVISION" }).click()
await page.getByText("MATCHES PROGRAM").waitFor({ timeout: 15_000 })
try {
  await page.waitForFunction(() => [...document.querySelectorAll("video")].every((video) => video.getVideoPlaybackQuality().totalVideoFrames > 10), null, { timeout: 30_000 })
} catch (error) {
  const diagnostics = {
    browserErrors,
    media: await page.locator("video").evaluateAll((videos) => videos.map((video) => ({
      label: video.getAttribute("aria-label"),
      readyState: video.readyState,
      width: video.videoWidth,
      height: video.videoHeight,
      frames: video.getVideoPlaybackQuality().totalVideoFrames,
    }))),
    peerStates: await page.evaluate(() => (globalThis.__liveProductionPeerConnections ?? []).map((peerConnection) => ({
      connectionState: peerConnection.connectionState,
      iceConnectionState: peerConnection.iceConnectionState,
      signalingState: peerConnection.signalingState,
    }))),
  }
  await writeFile(new URL("media-progress-failure.json", evidenceDirectory), `${JSON.stringify(diagnostics, null, 2)}\n`)
  await page.screenshot({ path: new URL("media-progress-failure.png", evidenceDirectory).pathname, fullPage: true })
  throw error
}

const initialMedia = await page.locator("video").evaluateAll((videos) => videos.map((video) => ({
  label: video.getAttribute("aria-label"),
  muted: video.muted,
  readyState: video.readyState,
  width: video.videoWidth,
  height: video.videoHeight,
  frames: video.getVideoPlaybackQuality().totalVideoFrames,
})))

await page.getByRole("button", { name: "setup" }).click()
await page.getByRole("button", { name: "Listen before mix", exact: true }).click()
await page.getByRole("button", { name: "Enable browser monitoring" }).click()
await page.waitForFunction(() => {
  const audio = document.querySelector("audio")
  return audio !== null && audio.srcObject instanceof MediaStream && audio.srcObject.getAudioTracks().length === 1 && !audio.muted
}, null, { timeout: 30_000 })
await waitForPrivateAudio(page)

const stats = await peerStats(page)
await writeFile(new URL("peer-stats.json", evidenceDirectory), `${JSON.stringify(stats, null, 2)}\n`)
const afterListenSnapshot = await page.evaluate(async () => (await fetch("/api/v1/snapshot")).json())
if (JSON.stringify(initialSnapshot.project.audioRoutes) !== JSON.stringify(afterListenSnapshot.project.audioRoutes)) {
  throw new Error("Private Listen changed the server audio mix")
}
if (!stats.some((entry) => entry.inbound.some((report) => report.kind === "video" && report.framesDecoded > 10))) {
  throw new Error("No WebRTC peer decoded changing video frames")
}
if (!stats.some((entry) => entry.privateSource && entry.inbound.some((report) => report.kind === "audio" && report.packetsReceived > 3))) {
  throw new Error("The selected private-source peer did not receive audio packets")
}

await page.screenshot({ path: new URL("desktop.png", evidenceDirectory).pathname, fullPage: true })
await page.setViewportSize({ width: 375, height: 812 })
await page.screenshot({ path: new URL("mobile.png", evidenceDirectory).pathname, fullPage: true })
const appliedRevision = afterListenSnapshot.program.programRevision
await page.close()

const observer = await context.newPage()
await observer.goto(baseUrl)
await observer.getByText("Untitled production").waitFor()
const afterCloseSnapshot = await observer.evaluate(async () => (await fetch("/api/v1/snapshot")).json())
if (afterCloseSnapshot.program.programRevision !== appliedRevision) {
  throw new Error("Closing the media page changed Program")
}

const evidence = {
  baseUrl,
  initialMedia,
  stats,
  browserErrors,
  programRevisionBeforeClose: appliedRevision,
  programRevisionAfterClose: afterCloseSnapshot.program.programRevision,
  privateListenPreservedAudioMix: true,
  beforeTakeColors,
  operatorControls,
}
await writeFile(new URL("media-stats.json", evidenceDirectory), `${JSON.stringify(evidence, null, 2)}\n`)
console.log(JSON.stringify(evidence, null, 2))
} catch (error) {
  if (page !== undefined && !page.isClosed()) {
    await page.screenshot({ path: new URL("workflow-failure.png", evidenceDirectory).pathname, fullPage: true })
    await writeFile(new URL("workflow-failure.json", evidenceDirectory), JSON.stringify({
      text: await page.locator("main").innerText(),
      media: await page.locator("video").evaluateAll(videos => videos.map(video => ({ readyState: video.readyState, frames: video.getVideoPlaybackQuality().totalVideoFrames }))),
    }, null, 2))
  }
  throw error
} finally {
  await browser.close()
}
