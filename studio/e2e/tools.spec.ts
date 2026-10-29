import { expect, test, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

// The Box and Select tools (#69) in a browser, over the studio's core:
// draw a box on top of another, resize it, lift it, and undo it all.

const S = 40;
const COS = 0.8660254037844387;

async function editing(page: Page, name: string) {
  await page.goto('/');
  await page.getByRole('button', { name: 'Open the sample library' }).click();
  await page.getByRole('tab', { name: 'Tree' }).click();
  await page.getByRole('list', { name: 'Icons' }).getByRole('button', { name, exact: true }).click();
  await expect(page.getByRole('application', { name: `Canvas: ${name}` })).toBeVisible();
  await page.getByRole('tab', { name: 'Source' }).click();
}

/** Where a tile point is on the page, by the canvas' camera now. */
async function at(page: Page, x: number, y: number, z: number): Promise<[number, number]> {
  const box = (await page.getByRole('application').boundingBox())!;
  const t = (await page.locator('[role=application] svg > g').getAttribute('transform'))!;
  const [tx, ty, k] = t.match(/-?[\d.]+(e-?\d+)?/g)!.map(Number) as [number, number, number];
  const X = (x - y) * COS * S;
  const Y = (x + y) * 0.5 * S - z * S;
  return [box.x + tx + X * k, box.y + ty + Y * k];
}

const scale = async (page: Page) => Number((await page.locator('[role=application] svg > g').getAttribute('transform'))!.match(/scale\(([^)]+)\)/)![1]);
const source = (page: Page) => page.getByRole('textbox', { name: /^Source of sample\.ilx/ });

test('a box drawn on top of another, resized, lifted, and undone', async ({ page }) => {
  await editing(page, 'switch-l3');
  const before = await source(page).inputValue();
  const k = await scale(page);

  // drawn on the chassis' top face: it stands on it
  await page.keyboard.press('b');
  await page.mouse.move(...(await at(page, 0.25, 0.25, 0.28)));
  await page.mouse.down();
  await page.mouse.move(...(await at(page, 0.75, 0.75, 0.28)), { steps: 4 });
  await expect(page.getByText('0.5 × 0.5', { exact: true })).toBeVisible();
  await page.mouse.up();
  const [hx, hy] = await at(page, 0.75, 0.75, 0.28);
  await page.mouse.move(hx, hy - 10 * k, { steps: 4 });
  await page.mouse.down();
  await page.mouse.up();
  await expect(source(page)).toHaveValue(/box 0\.25 0\.25 0\.28 0\.5 0\.5 0\.25\n/);
  // and it is selected
  await expect(page.locator('[role=application] .marked')).toHaveCount(3);
  expect((await new AxeBuilder({ page }).analyze()).violations.map((v) => v.id)).toEqual([]);

  // its far corner dragged out
  await page.keyboard.press('v');
  const corner = page.locator('[data-handle="corner-2"]');
  await corner.hover();
  await page.mouse.down();
  await page.mouse.move(...(await at(page, 1, 1, 0.28)), { steps: 6 });
  await page.mouse.up();
  await expect(source(page)).toHaveValue(/box 0\.25 0\.25 0\.28 0\.75 0\.75 0\.25\n/);

  // lifted a quarter, to the step
  // on its top face, clear of the height handle at its middle
  const [lx, ly] = await at(page, 0.4, 0.85, 0.53);
  await page.keyboard.down('Alt');
  await page.mouse.move(lx, ly);
  await page.mouse.down();
  await page.mouse.move(lx, ly - 10 * k, { steps: 6 });
  await page.mouse.up();
  await page.keyboard.up('Alt');
  await expect(source(page)).toHaveValue(/box 0\.25 0\.25 0\.5 0\.75 0\.75 0\.25\n/);

  // three steps, three undos
  for (let i = 0; i < 3; i++) await page.getByRole('button', { name: 'Undo' }).click();
  await expect(source(page)).toHaveValue(before);
});

test('an expression asks before it is changed', async ({ page }) => {
  await editing(page, 'switch-l3');
  const parts = page.getByRole('complementary', { name: 'Parts' });
  await parts.getByRole('button', { name: /^chassis\s*from rack-unit/ }).click();
  await parts.getByRole('button', { name: /^box/ }).click();
  const handle = page.locator('[data-handle="height"] polygon');
  const k = await scale(page);
  await handle.hover();
  await page.mouse.down();
  const b = (await handle.boundingBox())!;
  await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2 - 12 * k, { steps: 4 });
  await page.mouse.up();
  const dialog = page.getByRole('dialog', { name: 'This edit needs a choice' });
  await expect(dialog.getByRole('button', { name: /^Edit param u/ })).toBeVisible();
  await expect(dialog.getByRole('button', { name: /^Replace with a number/ })).toBeVisible();
  expect((await new AxeBuilder({ page }).analyze()).violations.map((v) => v.id)).toEqual([]);
  await dialog.getByRole('button', { name: 'Cancel' }).click();
  await expect(dialog).toHaveCount(0);
});
