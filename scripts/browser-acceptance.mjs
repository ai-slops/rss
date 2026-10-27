import { mkdir, writeFile } from "node:fs/promises"
import { chromium } from "../web/node_modules/playwright/index.mjs"
import { ensureSupervised } from './runner-supervision.mjs'
await ensureSupervised()

const baseUrl = process.env.LIVE_PRODUCTION_URL ?? "http://127.0.0.1:3001"
const password = process.env.LIVE_PRODUCTION_PASSWORD ?? "test-operator-password"
const evidenceDirectory = new URL("../.tools/browser-evidence/", import.meta.url)
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
  args: ["--no-sandbox", "--disable-dev-shm-usage", "--disable-gpu", "--autoplay-policy=no-user-gesture-required", "--use-fake-ui-for-media-stream", "--use-fake-device-for-media-stream", "--force-webrtc-ip-handling-policy=default", "--disable-features=WebRtcHideLocalIpsWithMdns,LocalNetworkAccessChecksWebRTC"],
})
try {
const context = await browser.newContext({ viewport: { width: 1280, height: 900 }, permissions: ["camera", "microphone"] })
const page = await context.newPage()
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
await page.getByRole("button", { name: "Add synthetic input" }).click()
await page.getByText("Synthetic bars").waitFor({ timeout: 15_000 })
await page.getByRole("button", { name: "Add video" }).click()
await page.getByRole("tab", { name: "Layers" }).click()
await page.getByText("Synthetic bars").last().waitFor({ timeout: 15_000 })
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
await page.getByRole("button", { name: "Listen" }).click()
await page.getByRole("button", { name: "Enable browser monitoring" }).click()
await page.waitForFunction(() => {
  const audio = document.querySelector("audio")
  return audio !== null && audio.srcObject instanceof MediaStream && audio.srcObject.getAudioTracks().length === 1 && !audio.muted
}, null, { timeout: 30_000 })

const stats = await page.evaluate(async () => {
  const peerConnections = globalThis.__liveProductionPeerConnections ?? []
  const reports = []
  for (const peerConnection of peerConnections) {
    const values = []
    for (const report of (await peerConnection.getStats()).values()) {
      if (report.type === "inbound-rtp") {
        values.push({
          kind: report.kind,
          bytesReceived: report.bytesReceived,
          packetsReceived: report.packetsReceived,
          framesDecoded: report.framesDecoded,
          jitter: report.jitter,
        })
      }
    }
    reports.push({ connectionState: peerConnection.connectionState, iceConnectionState: peerConnection.iceConnectionState, inbound: values })
  }
  return reports
})
const afterListenSnapshot = await page.evaluate(async () => (await fetch("/api/v1/snapshot")).json())
if (JSON.stringify(initialSnapshot.project.audioRoutes) !== JSON.stringify(afterListenSnapshot.project.audioRoutes)) {
  throw new Error("Private Listen changed the server audio mix")
}
if (!stats.some((entry) => entry.inbound.some((report) => report.kind === "video" && report.framesDecoded > 10))) {
  throw new Error("No WebRTC peer decoded changing video frames")
}
if (!stats.some((entry) => entry.inbound.some((report) => report.kind === "audio" && report.packetsReceived > 0))) {
  throw new Error("No WebRTC peer received audio packets")
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
}
await writeFile(new URL("media-stats.json", evidenceDirectory), `${JSON.stringify(evidence, null, 2)}\n`)
console.log(JSON.stringify(evidence, null, 2))
} finally {
  await browser.close()
}
