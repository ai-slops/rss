import assert from 'node:assert/strict';
import { expect } from '../web/node_modules/playwright/test.mjs';

export async function exerciseOperatorControls(page) {
  const snapshot = () => page.evaluate(async () => (await fetch('/api/v1/snapshot')).json());
  const initial = await snapshot();
  const sceneId = initial.preview.sceneId;
  const itemId = initial.preview.scene.items[0].id;
  const item = async () => (await snapshot()).project.scenes.find(scene => scene.id === sceneId).items.find(item => item.id === itemId);

  await page.locator('.canvas-overlay__hit').first().click({ position: { x: 40, y: 40 } });
  await page.waitForResponse(response => response.url().endsWith('/api/v1/snapshot') && response.status() === 200);
  assert.deepEqual((await item()).rect, initial.preview.scene.items[0].rect, 'Selection-only click changed geometry');
  await page.getByLabel('X', { exact: true }).fill('37');
  await page.getByLabel('X', { exact: true }).press('Tab');
  await expect.poll(async () => (await item()).rect.x).toBe(37);
  await page.waitForResponse(response => response.url().endsWith('/api/v1/snapshot') && response.status() === 200);
  assert.equal(await page.getByLabel('X', { exact: true }).count(), 1, 'Saved edit cleared layer selection');
  await page.getByLabel('Left', { exact: true }).fill('0.1');
  await page.getByLabel('Left', { exact: true }).press('Tab');
  await expect.poll(async () => (await item()).crop.left).toBe(0.1);
  await page.getByRole('slider', { name: /^Opacity/ }).press('ArrowLeft');
  await expect.poll(async () => (await item()).opacity).toBe(0.99);
  const beforeEscape = (await item()).rect;
  const target = await page.locator('.canvas-overlay__hit').first().boundingBox();
  assert.notEqual(target, null, 'Canvas item has no visible target');
  await page.mouse.move(target.x + target.width / 2, target.y + target.height / 2);
  await page.mouse.down();
  await page.mouse.move(target.x + target.width / 2 + 16, target.y + target.height / 2 + 12);
  await page.keyboard.press('Escape');
  await page.mouse.up();
  await page.waitForResponse(response => response.url().endsWith('/api/v1/snapshot') && response.status() === 200);
  assert.deepEqual((await item()).rect, beforeEscape, 'Immediate Escape committed a cancelled drag');
  const otherView = await page.context().newPage();
  try {
    await otherView.goto(page.url());
    await otherView.locator('.workspace').waitFor();
    await otherView.getByRole('tab', { name: 'Layers', exact: true }).click();
    await otherView.locator('.layer-row__select').first().click();
    await otherView.getByLabel('X', { exact: true }).fill('73');
    await otherView.getByLabel('X', { exact: true }).press('Tab');
    await expect.poll(async () => (await item()).rect.x).toBe(73);
    await expect.poll(() => page.getByLabel('X', { exact: true }).inputValue()).toBe('73');
    await page.locator('.canvas-overlay').focus();
    await page.keyboard.press('Control+z');
    await page.waitForResponse(response => response.url().endsWith('/api/v1/snapshot') && response.status() === 200);
    assert.equal((await item()).rect.x, 73, 'Stale undo overwrote the other view edit');
  } finally {
    await otherView.close();
  }
  await page.getByRole('button', { name: 'sync', exact: true }).click();
  await page.getByRole('spinbutton', { name: 'Synthetic bars video correction', exact: true }).fill('20');
  await page.getByRole('button', { name: 'Validate and apply', exact: true }).click();
  await expect.poll(async () => (await snapshot()).project.sources[0].tracks.find(track => track.kind === 'video').correctionMs).toBe(20);
  await page.getByRole('button', { name: 'setup', exact: true }).click();

  await page.getByRole('button', { name: 'Audio routes', exact: true }).click();
  const checkbox = page.locator('.production-setup__route input[type=checkbox]').first();
  await checkbox.check();
  await page.getByRole('button', { name: 'Apply routes', exact: true }).click();
  await expect.poll(async () => (await snapshot()).project.audioRoutes.length).toBe(1);
  await checkbox.uncheck();
  await page.getByRole('button', { name: 'Apply routes', exact: true }).click();
  await expect.poll(async () => (await snapshot()).project.audioRoutes.length).toBe(0);
  await page.getByRole('button', { name: 'Inputs', exact: true }).click();
  return { selectionClickPreservedGeometry: true, savedEditPreservedSelection: true, numericCropAndOpacityApplied: true, immediateEscapeCancelledDrag: true, staleUndoPreservedOtherViewEdit: true, liveTimingControlApplied: true, explicitRouteAndUnrouteApplied: true };
}
