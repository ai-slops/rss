import assert from 'node:assert/strict';
import test from 'node:test';
import { chromium } from '../web/node_modules/playwright/index.mjs';
import { waitForPrivateAudio } from './browser-media-evidence.mjs';
import { ensureSupervised } from './runner-supervision.mjs';

await ensureSupervised();

test('private audio wait rejects when no source stream exists', async () => {
  const browser = await chromium.launch({
    executablePath: new URL('./browser-chromium.sh', import.meta.url).pathname,
    headless: true,
    args: ['--no-sandbox', '--disable-dev-shm-usage'],
  });
  try {
    const page = await browser.newPage();
    await page.setContent('<audio aria-label="Private source monitor"></audio>');
    await assert.rejects(waitForPrivateAudio(page, 200));
  } finally {
    await browser.close();
  }
});
