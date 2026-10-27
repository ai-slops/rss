import { chromium, errors } from '../web/node_modules/playwright/index.mjs';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import assert from 'node:assert/strict';

const origin = process.env.LIVE_PRODUCTION_URL ?? 'https://live-production--studio-6d4ac2cc57021bab.mj-y.com/';
const password = (await readFile(new URL('../var/operator-password', import.meta.url), 'utf8')).trimEnd();
const directory = fileURLToPath(new URL(`../var/browser-review/capture-${Date.now()}/`, import.meta.url));
await mkdir(directory, { recursive: true });
const browser = await chromium.launch({
  executablePath: fileURLToPath(new URL('./browser-chromium.sh', import.meta.url)),
  headless: true,
});
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  let consoleErrors = 0;
  page.on('console', message => { if (message.type() === 'error') consoleErrors += 1; });
  const response = await page.goto(origin);
  assert.equal(response?.status(), 200);
  await page.getByLabel('Operator password').fill(password);
  await page.getByRole('button', { name: 'Open control room' }).click();
  await page.locator('.workspace').waitFor();
  let decodedMediaObserved = true;
  try {
    await page.waitForFunction(() => {
      const videos = [...document.querySelectorAll('video')];
      return videos.length >= 2 && videos.every(video => video.getVideoPlaybackQuality().totalVideoFrames > 0);
    }, null, { timeout: 15_000 });
  } catch (error) {
    if (!(error instanceof errors.TimeoutError)) throw error;
    decodedMediaObserved = false;
  }
  const captures = [];
  const capture = async name => {
    const path = `${directory}/${name}.png`;
    await page.screenshot({ path, fullPage: true });
    captures.push(path);
  };
  await capture('production');
  for (const tab of ['audio', 'sync', 'outputs', 'setup']) {
    await page.getByRole('navigation', { name: 'Production operations' }).getByRole('button', { name: tab, exact: true }).click();
    await capture(tab);
  }
  for (const tab of ['inputs', 'assets', 'recordings', 'plugins']) {
    await page.getByRole('navigation', { name: 'Setup sections' }).getByRole('button', { name: tab, exact: true }).click();
    await capture(`setup-${tab}`);
  }
  const media = await page.locator('video').evaluateAll(videos => videos.map(video => ({
    readyState: video.readyState,
    currentTime: video.currentTime,
    frames: video.getVideoPlaybackQuality().totalVideoFrames,
    width: video.videoWidth,
    height: video.videoHeight,
    muted: video.muted,
  })));
  const report = { origin, viewport: { width: 1280, height: 900 }, captures, media, decodedMediaObserved, consoleErrors,
    limitations: ['Read-only control/navigation captures; this script does not establish complete production/media acceptance.'] };
  await writeFile(`${directory}/report.json`, `${JSON.stringify(report, null, 2)}\n`);
  console.info(JSON.stringify({ captures: captures.length, directory, media, decodedMediaObserved, consoleErrors }));
} finally {
  await browser.close();
}
