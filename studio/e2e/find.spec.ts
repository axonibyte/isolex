import { expect, test, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

// Find (#117) in a browser, over the studio's core: the library searched,
// a result opening its place, and axe over the dialog in both themes.

const violations = async (page: Page) => (await new AxeBuilder({ page }).analyze()).violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' ')).join(', ')}`);

async function finding(page: Page, query: string) {
  await page.goto('/');
  await page.getByRole('button', { name: 'Open the sample library' }).click();
  await expect(page.getByRole('application', { name: /^Canvas: / })).toBeVisible();
  await page.keyboard.press('Control+f');
  const dialog = page.getByRole('dialog', { name: 'Find' });
  await dialog.getByRole('searchbox', { name: 'Find in the library' }).fill(query);
  return dialog;
}

for (const [theme, canvas] of [
  ['Dark', 'Light canvas'],
  ['Light', 'Dark canvas'],
] as const) {
  test(`Find is accessible: ${theme}, ${canvas.toLowerCase()}`, async ({ page }) => {
    await page.goto('/');
    await page.getByRole('button', { name: 'Open the sample library' }).click();
    await page.getByRole('button', { name: theme, exact: true }).click();
    await page.getByRole('button', { name: canvas }).click();
    await page.keyboard.press('Control+f');
    await page.getByRole('searchbox', { name: 'Find in the library' }).fill('state');
    await expect(page.getByRole('list', { name: /^Parts/ })).toBeVisible();
    expect(await violations(page)).toEqual([]);
  });
}

test('a part found opens its definition and its line', async ({ page }, info) => {
  const dialog = await finding(page, 'ports');
  await page.screenshot({ path: info.outputPath('find.png') });
  await dialog.getByRole('list', { name: 'Parts (2)' }).getByRole('button').nth(1).click();
  await expect(page.getByRole('application', { name: 'Canvas: switch-poe' })).toBeVisible();
  const source = page.getByRole('textbox', { name: /^Source of sample\.ilx/ });
  await expect.poll(() => source.evaluate((t: HTMLTextAreaElement) => t.value.slice(t.selectionStart, t.selectionStart + 15))).toBe('replace "ports"');
});
