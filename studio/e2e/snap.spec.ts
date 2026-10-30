import { expect, test, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

// Advanced snapping (#118) in a browser, over the studio's core: the
// Advanced list beside the step, axe over it in both themes, and an
// alignment guide drawn while measuring, gone with guides switched off.

const S = 40;
const COS = 0.8660254037844387;

const violations = async (page: Page) => (await new AxeBuilder({ page }).analyze()).violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' ')).join(', ')}`);

async function editing(page: Page, name: string) {
  await page.goto('/');
  await page.getByRole('button', { name: 'Open the sample library' }).click();
  await page.getByRole('tab', { name: 'Tree' }).click();
  await page.getByRole('list', { name: 'Icons' }).getByRole('button', { name, exact: true }).click();
  await expect(page.getByRole('application', { name: `Canvas: ${name}` })).toBeVisible();
}

/** The camera's offset and scale now. */
async function camera(page: Page): Promise<[number, number, number]> {
  const t = (await page.locator('[role=application] svg > g').getAttribute('transform'))!;
  return t.match(/-?[\d.]+(e-?\d+)?/g)!.map(Number) as [number, number, number];
}

/** Where a tile point on the floor is on the page. */
async function at(page: Page, x: number, y: number): Promise<[number, number]> {
  const box = (await page.getByRole('application').boundingBox())!;
  const [tx, ty, k] = await camera(page);
  return [box.x + tx + (x - y) * COS * S * k, box.y + ty + (x + y) * 0.5 * S * k];
}

for (const [theme, canvas] of [
  ['Dark', 'Light canvas'],
  ['Light', 'Dark canvas'],
] as const) {
  test(`the Advanced list is accessible: ${theme}, ${canvas.toLowerCase()}`, async ({ page }) => {
    await page.goto('/');
    await page.getByRole('button', { name: 'Open the sample library' }).click();
    await page.getByRole('button', { name: theme, exact: true }).click();
    await page.getByRole('button', { name: canvas }).click();
    await page.getByRole('button', { name: 'Advanced' }).click();
    await expect(page.getByRole('group', { name: 'Snap to' }).getByRole('checkbox')).toHaveCount(9);
    expect(await violations(page)).toEqual([]);
  });
}

test('a guide drawn while a point lines up, and none switched off', async ({ page }, info) => {
  await editing(page, 'switch-poe');
  await page.getByRole('button', { name: 'Advanced' }).click();
  const list = page.getByRole('group', { name: 'Snap to' });
  // off the grid, so only the guide pulls
  await list.getByRole('checkbox', { name: 'Grid lines and intersections' }).uncheck();
  const k = (await camera(page))[2];
  // three pixels right of the line through the tile's corners at x 2,
  // below the tile, away from its edges
  const off = 3 / (COS * S * k);

  await page.getByRole('button', { name: 'Measure', exact: true }).click();
  await page.mouse.move(...(await at(page, 1.5, 1.6)));
  await page.mouse.down();
  await page.mouse.move(...(await at(page, 2 + off, 1.6)), { steps: 4 });
  await expect(page.locator('.tag.snap')).toHaveText(/^aligned with /);
  await expect(page.locator('[role=application] line.aligned')).toHaveCount(1);
  await expect(page.getByText('0.5 tiles · 0° · Δx 0.5 Δy 0', { exact: true })).toBeVisible();
  await page.screenshot({ path: info.outputPath('guide.png') });
  await page.mouse.up();

  await list.getByRole('checkbox', { name: 'Alignment guides' }).uncheck();
  await page.getByRole('button', { name: 'Measure', exact: true }).click();
  await page.mouse.move(...(await at(page, 1.5, 1.6)));
  await page.mouse.down();
  await page.mouse.move(...(await at(page, 2 + off, 1.6)), { steps: 4 });
  await expect(page.locator('[role=application] line.aligned')).toHaveCount(0);
  await expect(page.getByText(/^0\.5 tiles/)).toHaveCount(0);
  await page.mouse.up();
});
