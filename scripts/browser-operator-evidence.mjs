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

  await page.getByRole('button', { name: 'Audio routes', exact: true }).click();
  const checkbox = page.locator('.production-setup__route input[type=checkbox]').first();
  await checkbox.check();
  await page.getByRole('button', { name: 'Apply routes', exact: true }).click();
  await expect.poll(async () => (await snapshot()).project.audioRoutes.length).toBe(1);
  await checkbox.uncheck();
  await page.getByRole('button', { name: 'Apply routes', exact: true }).click();
  await expect.poll(async () => (await snapshot()).project.audioRoutes.length).toBe(0);
  await page.getByRole('button', { name: 'Inputs', exact: true }).click();
  return { selectionClickPreservedGeometry: true, savedEditPreservedSelection: true, numericCropAndOpacityApplied: true, immediateEscapeCancelledDrag: true, explicitRouteAndUnrouteApplied: true };
}
