import { expect, test, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

// Selecting in each of the three places, and opening a parent from an
// inherited part (#68), over the sample library.

async function editing(page: Page, name: string) {
  await page.goto('/');
  await page.getByRole('button', { name: 'Open the sample library' }).click();
  await page.getByRole('tab', { name: 'Tree' }).click();
  await page.getByRole('list', { name: 'Icons' }).getByRole('button', { name, exact: true }).click();
  await expect(page.getByRole('application', { name: `Canvas: ${name}` })).toBeVisible();
  await page.getByRole('tab', { name: 'Source' }).click();
  return page.getByRole('complementary', { name: 'Parts' });
}

const row = (parts: ReturnType<Page['getByRole']>, name: RegExp) => parts.getByRole('button', { name });
const highlighted = (page: Page) => page.locator('.ln.hl .num');

test('a row selects its shapes and its line', async ({ page }) => {
  const parts = await editing(page, 'switch-l3');
  await row(parts, /^path/).click();
  await expect(row(parts, /^path/)).toHaveAttribute('aria-current', 'true');
  await expect(page.locator('[role=application] .marked')).toHaveCount(1);
  await expect(highlighted(page)).toHaveText('41');
  expect((await new AxeBuilder({ page }).analyze()).violations.map((v) => v.id)).toEqual([]);
});

test('a click on the canvas selects; an inherited shape says where it is from', async ({ page }) => {
  const parts = await editing(page, 'switch-l3');
  const marked = page.locator('[role=application] .marked');
  // the routing arrows, drawn last, on top
  await page.locator('[role=application] [data-shape]').last().click();
  await expect(row(parts, /^path/)).toHaveAttribute('aria-current', 'true');
  await expect(marked).toHaveCount(1);
  // the chassis, the first shape drawn, is rack-unit's; others are drawn
  // over most of it, so the pointer moves on it as such
  await page.locator('[role=application] [data-shape="0"]').dispatchEvent('pointermove');
  await expect(page.getByText(/^From rack-unit, part chassis/)).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(marked).toHaveCount(0);
});

test('the caret in the source selects', async ({ page }) => {
  const parts = await editing(page, 'switch-l3');
  const source = page.getByRole('textbox', { name: /^Source of sample\.ilx/ });
  await source.click();
  await page.keyboard.press('Control+Home');
  for (let i = 0; i < 40; i++) await page.keyboard.press('ArrowDown');
  // into the path's text: the end of its line is past it, in its at
  await page.keyboard.press('End');
  await page.keyboard.press('ArrowLeft');
  await expect(row(parts, /^path/)).toHaveAttribute('aria-current', 'true');
  await expect(page.locator('[role=application] .marked')).toHaveCount(1);
});

test('an inherited part opens the definition it comes from', async ({ page }) => {
  const parts = await editing(page, 'switch-l3');
  await row(parts, /^chassis\s*from rack-unit/).click();
  await expect(page.getByRole('banner')).toContainText(/base\s*rack-unit\s*as drawn in\s*switch-l3/);
  await expect(row(parts, /^chassis$/)).toBeVisible();
  await row(parts, /^box/).click();
  await expect(page.locator('[role=application] .marked')).toHaveCount(3);
  await expect(highlighted(page)).toHaveText('15');
  // and the switch's own icon opens it back
  await page.getByRole('tab', { name: 'Tree' }).click();
  await page.getByRole('list', { name: 'Icons' }).getByRole('button', { name: 'switch-l3', exact: true }).click();
  await expect(row(parts, /^routing-badge/)).toBeVisible();
});
