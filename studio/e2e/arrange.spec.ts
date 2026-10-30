import { expect, test, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

// Align and Distribute (#114) in a browser, over the studio's core: shapes
// chosen with Shift in the Parts panel, lined up, and axe over the panel
// in both themes.

const violations = async (page: Page) => (await new AxeBuilder({ page }).analyze()).violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' ')).join(', ')}`);

async function chosen(page: Page) {
  await page.goto('/');
  await page.getByRole('button', { name: 'Open the sample library' }).click();
  await expect(page.getByRole('application', { name: /^Canvas: / })).toBeVisible();
  await page.getByRole('tab', { name: 'Tree' }).click();
  await page.getByRole('button', { name: /^switch-poe( |$)/ }).click();
  await expect(page.getByRole('application', { name: 'Canvas: switch-poe' })).toBeVisible();
  const parts = page.getByRole('complementary', { name: 'Parts' });
  await parts.getByRole('button', { name: /^tile/ }).click();
  await parts.getByRole('button', { name: /^cylinder/ }).click({ modifiers: ['Shift'] });
  await parts.getByRole('button', { name: /^shadow/ }).click({ modifiers: ['Shift'] });
  await page.keyboard.press('Control+Shift+A');
  const panel = page.getByRole('tabpanel');
  await expect(panel.getByText(/^3 shapes/)).toBeVisible();
  return panel;
}

for (const [theme, canvas] of [
  ['Dark', 'Light canvas'],
  ['Light', 'Dark canvas'],
] as const) {
  test(`Align and Distribute is accessible: ${theme}, ${canvas.toLowerCase()}`, async ({ page }) => {
    await chosen(page);
    await page.getByRole('button', { name: theme, exact: true }).click();
    await page.getByRole('button', { name: canvas }).click();
    expect(await violations(page)).toEqual([]);
  });
}

test('lines up the shapes chosen, as one undo step', async ({ page }, info) => {
  const panel = await chosen(page);
  await page.screenshot({ path: info.outputPath('arrange.png') });
  // the tile's x is 0, the cylinder's and the shadow's left edges past it
  await panel.getByRole('button', { name: 'Align x: min' }).click();
  await expect(page.getByText('Aligned 2 shapes')).toBeVisible();
  await page.getByRole('tab', { name: 'Source' }).click();
  const source = page.getByRole('textbox', { name: /^Source of sample\.ilx/ });
  await expect(source).toHaveValue(/cylinder 0\.4 0\.5 0 0\.4 0\.12/);
  await expect(source).toHaveValue(/shadow 0\.48 0\.5 0\.48 0\.48/);
  await page.getByRole('button', { name: 'Undo' }).click();
  await expect(source).toHaveValue(/cylinder 0\.5 0\.5 0 0\.4 0\.12/);
  await expect(source).toHaveValue(/shadow 0\.5 0\.5 0\.48 0\.48/);
});

test('rotates a shape on the screen, written as a transform and drawn so', async ({ page }, info) => {
  const panel = await chosen(page);
  const parts = page.getByRole('complementary', { name: 'Parts' });
  await parts.getByRole('button', { name: /^tile/ }).click();
  await panel.getByRole('button', { name: 'Rotate clockwise' }).click();
  await expect(page.getByText('Rotated one shape')).toBeVisible();
  // the canvas draws the tile through the map, as the language says
  await expect(page.getByRole('application').locator('g[transform^="matrix("]').first()).toBeAttached();
  await page.screenshot({ path: info.outputPath('transform.png') });
  await page.getByRole('tab', { name: 'Source' }).click();
  await expect(page.getByRole('textbox', { name: /^Source of sample\.ilx/ })).toHaveValue(/transform rotate=15 origin="[-\d. ]+" \{\n {6}tile 0 0 0 2 1/);
});

test('a second click shows turn handles, and dragging a corner rotates', async ({ page }, info) => {
  await chosen(page);
  const parts = page.getByRole('complementary', { name: 'Parts' });
  await parts.getByRole('button', { name: /^tile/ }).click();
  const canvas = page.getByRole('application');
  const tile = canvas.locator('.marked').first();
  const box = (await tile.boundingBox())!;
  // a click on the tile selected: its turn handles
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  const corner = canvas.locator('[data-handle="rot-1"]');
  await expect(corner).toBeVisible();
  await page.screenshot({ path: info.outputPath('handles.png') });
  // the top-right corner dragged a quarter round, clockwise, about the middle
  const c = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
  const at = (await corner.boundingBox())!;
  const from = { x: at.x + at.width / 2, y: at.y + at.height / 2 };
  const to = { x: c.x - (from.y - c.y), y: c.y + (from.x - c.x) };
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move((from.x + to.x) / 2 + 10, (from.y + to.y) / 2 + 10, { steps: 4 });
  await page.mouse.move(to.x, to.y, { steps: 4 });
  await expect(page.getByText(/^rotate 90°$/)).toBeVisible();
  await page.screenshot({ path: info.outputPath('turning.png') });
  await page.mouse.up();
  await page.getByRole('tab', { name: 'Source' }).click();
  await expect(page.getByRole('textbox', { name: /^Source of sample\.ilx/ })).toHaveValue(/transform rotate=90 origin="[-\d. ]+" \{\n {6}tile 0 0 0 2 1/);
});
