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
