import { mkdir } from "node:fs/promises"
import { chromium } from "../web/node_modules/playwright/index.mjs"
import { ensureSupervised } from "./runner-supervision.mjs"

await ensureSupervised()

const baseUrl = process.env.LIVE_PRODUCTION_URL ?? "http://127.0.0.1:3096"
const password = process.env.LIVE_PRODUCTION_PASSWORD ?? "output-browser-password"
const evidenceDirectory = process.env.OUTPUT_BROWSER_EVIDENCE ?? "var/output-browser-gate"
const browserRoot = new URL("../.tools/browser-root/", import.meta.url).pathname
await mkdir(evidenceDirectory, { recursive: true })

async function openControlRoom(page) {
  const passwordField = page.getByLabel("Operator password")
  if (await passwordField.isVisible()) {
    await passwordField.fill(password)
    await page.getByRole("button", { name: "Open control room" }).click()
  }
  await page.getByText("Untitled production").waitFor({ timeout: 60_000 })
}

const browser = await chromium.launch({
  executablePath: new URL("./browser-chromium.sh", import.meta.url).pathname,
  env: {
    ...process.env,
    LD_LIBRARY_PATH: `${browserRoot}usr/lib/pulseaudio:${browserRoot}usr/lib:${browserRoot}lib`,
    FONTCONFIG_PATH: `${browserRoot}etc/fonts`,
    XDG_DATA_DIRS: `${browserRoot}usr/share`,
  },
  headless: true,
  args: ["--no-sandbox", "--disable-dev-shm-usage", "--disable-gpu", "--no-zygote", "--single-process"],
})

try {
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 } })
  const page = await context.newPage()
  const errors = []
  page.on("pageerror", (error) => errors.push(error.message))
  await page.goto(baseUrl)
  await openControlRoom(page)
  await page.getByRole("button", { name: "outputs" }).click()
  await page.evaluate(async () => {
    const snapshot = await (await fetch("/api/v1/snapshot")).json()
    const response = await fetch("/api/v1/commands", {
      method: "POST",
      headers: { "content-type": "application/json", "x-live-production": "1" },
      body: JSON.stringify({
        id: "browser-source",
        kind: "source.create",
        targetId: snapshot.project.id,
        expectedEngineRunId: snapshot.engineRunId,
        payload: { name: "Synthetic bars", adapter: { kind: "synthetic", pattern: "smpte", frequencyHz: 440 } },
      }),
    })
    const receipt = await response.json()
    if (!response.ok || receipt.status !== "applied") {
      throw new Error(`source creation failed: ${response.status} ${JSON.stringify(receipt)}`)
    }
  })
  await page.reload()
  await openControlRoom(page)
  await page.getByRole("button", { name: "outputs" }).click()
  await page.getByRole("option", { name: "Synthetic bars" }).waitFor({ state: "attached" })

  await page.getByRole("button", { name: "Add Program recorder" }).click()
  const programRow = page.locator(".output-row").filter({ hasText: "Program recording" }).last()
  await programRow.waitFor()
  await programRow.getByRole("button", { name: "Start" }).click()
  await page.waitForFunction(async () => {
    const snapshot = await (await fetch("/api/v1/snapshot")).json()
    return snapshot.outputStatuses.some((status) => status.actual === "running")
  }, null, { timeout: 15_000 })
  await page.waitForTimeout(2_000)
  await page.reload()
  await openControlRoom(page)
  await page.getByRole("button", { name: "outputs" }).click()
  await programRow.getByRole("button", { name: "Stop" }).click()
  await page.waitForFunction(async () => {
    const snapshot = await (await fetch("/api/v1/snapshot")).json()
    return snapshot.outputStatuses.some((status) => status.actual === "stopped")
  }, null, { timeout: 20_000 })

  await page.getByRole("button", { name: "Add clean recorder" }).click()
  await page.locator(".output-row").filter({ hasText: "Clean source recording" }).last().waitFor()
  await page.evaluate(async () => {
    const response = await fetch("/api/v1/output-destinations", {
      method: "POST",
      headers: { "content-type": "application/json", "x-live-production": "1" },
      body: JSON.stringify({ destinationId: "browser-local", url: "rtmps://localhost:19999/live/test" }),
    })
    if (!response.ok) throw new Error(`destination provisioning failed: ${response.status}`)
  })
  await page.getByPlaceholder("destination-id").fill("browser-local")
  await page.getByRole("button", { name: "Add RTMPS destination" }).click()
  await page.locator(".output-row").filter({ hasText: "RTMPS destination" }).last().waitFor()

  await page.screenshot({ path: `${evidenceDirectory}/outputs-1280.png`, fullPage: true })
  const desktopOverflow = await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth)
  if (desktopOverflow) throw new Error("desktop output controls overflow the viewport")
  await page.setViewportSize({ width: 375, height: 812 })
  await page.screenshot({ path: `${evidenceDirectory}/outputs-375.png`, fullPage: true })
  const mobileOverflow = await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth)
  if (mobileOverflow) throw new Error("mobile output controls overflow the viewport")
  if (errors.length > 0) throw new Error(`browser errors: ${errors.join(" | ")}`)
  process.stdout.write("output_browser_gate=passed\n")
} finally {
  await browser.close()
}
