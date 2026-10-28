import { expect, test, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

async function open(page: Page) {
  await page.goto('/');
  await page.getByRole('button', { name: 'Open the sample library' }).click();
  await expect(page.getByRole('application', { name: /^Canvas: / })).toBeVisible();
}

const violations = async (page: Page) => (await new AxeBuilder({ page }).analyze()).violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' ')).join(', ')}`);

test('the welcome is accessible', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('button', { name: 'Open library folder…' })).toBeVisible();
  expect(await violations(page)).toEqual([]);
});

for (const [theme, canvas] of [
  ['Dark', 'Light canvas'],
  ['Light', 'Dark canvas'],
  ['System', 'Light canvas'],
] as const) {
  test(`the frame is accessible: ${theme}, ${canvas.toLowerCase()}`, async ({ page }) => {
    await open(page);
    await page.getByRole('button', { name: theme, exact: true }).click();
    await page.getByRole('button', { name: canvas }).click();
    for (const tab of ['Source', 'Tree', 'Design', 'Preview']) {
      await page.getByRole('tab', { name: tab }).click();
      expect(await violations(page), tab).toEqual([]);
    }
  });
}

test('the themes are remembered', async ({ page }) => {
  await open(page);
  await page.getByRole('button', { name: 'Light', exact: true }).click();
  await page.getByRole('button', { name: 'Dark canvas' }).click();
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await expect(page.locator('html')).toHaveAttribute('data-canvas', 'dark');
});

test('the frame fits the window, however long the source', async ({ page }) => {
  await open(page);
  const viewport = page.viewportSize()!;
  const canvas = (await page.getByRole('application').boundingBox())!;
  expect(canvas.y + canvas.height).toBeLessThanOrEqual(viewport.height);
  const status = (await page.getByRole('contentinfo').boundingBox())!;
  expect(status.y + status.height).toBeLessThanOrEqual(viewport.height);
});

test('the wheel zooms, and with Ctrl pans', async ({ page }) => {
  await open(page);
  const drawing = page.locator('[role=application] svg > g');
  const at = async () => drawing.getAttribute('transform');
  const canvas = page.getByRole('application');
  await canvas.hover();
  const before = await at();
  await page.mouse.wheel(0, -200);
  await expect.poll(at).not.toBe(before);
  const zoomed = await at();
  await page.keyboard.down('Control');
  await page.mouse.wheel(0, 100);
  await page.keyboard.up('Control');
  await expect.poll(at).not.toBe(zoomed);
  // panning keeps the scale
  expect((await at())!.match(/scale\(([^)]+)\)/)![1]).toBe(zoomed!.match(/scale\(([^)]+)\)/)![1]);
  await page.keyboard.press('0');
  await expect.poll(at).toBe(before);
});

test('typing in the Source tab makes an undo step', async ({ page }) => {
  await open(page);
  const source = page.getByRole('textbox', { name: /^Source of sample\.ilx/ });
  await source.click();
  await page.keyboard.press('Control+End');
  await page.keyboard.type('\n// a note ');
  await expect(page.getByRole('contentinfo')).toContainText('unsaved changes');
  await page.getByRole('button', { name: 'Undo' }).click();
  await page.getByRole('button', { name: 'Undo' }).click();
  await expect(page.getByRole('contentinfo')).toContainText('saved');
  await expect(source).not.toHaveValue(/a note/);
});
