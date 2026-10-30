import { expect, test, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

// The Tree's focus view (#113) in a browser, over the studio's core: its
// cards and lines laid out as columns, a click recentering it, and axe
// over it in both themes.

const violations = async (page: Page) => (await new AxeBuilder({ page }).analyze()).violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' ')).join(', ')}`);

async function focused(page: Page) {
  await page.goto('/');
  await page.getByRole('button', { name: 'Open the sample library' }).click();
  await expect(page.getByRole('application', { name: /^Canvas: / })).toBeVisible();
  await page.getByRole('tab', { name: 'Tree' }).click();
  await page.getByRole('button', { name: /^switch-poe( |$)/ }).click();
  await expect(page.getByRole('application', { name: 'Canvas: switch-poe' })).toBeVisible();
  await page.getByRole('radio', { name: 'Focus' }).check();
}

for (const [theme, canvas] of [
  ['Dark', 'Light canvas'],
  ['Light', 'Dark canvas'],
] as const) {
  test(`the focus view is accessible: ${theme}, ${canvas.toLowerCase()}`, async ({ page }) => {
    await focused(page);
    await page.getByRole('button', { name: theme, exact: true }).click();
    await page.getByRole('button', { name: canvas }).click();
    await expect(page.locator('[aria-current="true"][data-key]')).toBeVisible();
    expect(await violations(page)).toEqual([]);
  });
}

test('the focus view lays its columns out left to right, lined, and recenters on a click', async ({ page }, info) => {
  await focused(page);
  const card = (name: string) => page.locator(`[data-key$=":${name}"]`);
  const box = async (name: string) => (await card(name).boundingBox())!;
  // what switch-poe depends on is to its left, a step further out further left
  const [poe, l2, sw] = [await box('switch-poe'), await box('switch-l2'), await box('switch')];
  expect(l2.x + l2.width).toBeLessThan(poe.x);
  expect(sw.x + sw.width).toBeLessThan(l2.x);
  // a line for each reference shown: five from switch-poe, two a step out
  await expect(page.locator('svg.lines path')).toHaveCount(7);
  await page.screenshot({ path: info.outputPath('focus.png') });

  await card('switch-l2').click();
  await expect(page.getByRole('application', { name: 'Canvas: switch-l2' })).toBeVisible();
  await expect(page.locator('[aria-current="true"][data-key]')).toHaveAttribute('data-key', '0:switch-l2');
  // what depends on it is to its right
  const [l2b, l3] = [await box('switch-l2'), await box('switch-l3')];
  expect(l3.x).toBeGreaterThan(l2b.x + l2b.width);
  // the middle kept in view
  await expect(card('switch-l2')).toBeInViewport();
});
