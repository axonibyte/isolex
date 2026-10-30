import { expect, test, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

// Insert and Badge (#116) in a browser, over the studio's core: a fragment
// dragged from the Insert tool's list onto the canvas, written where it
// is dropped, and axe over the list in both themes.

const violations = async (page: Page) => (await new AxeBuilder({ page }).analyze()).violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' ')).join(', ')}`);

async function inserting(page: Page) {
  await page.goto('/');
  await page.getByRole('button', { name: 'Open the sample library' }).click();
  await expect(page.getByRole('application', { name: /^Canvas: / })).toBeVisible();
  await page.getByRole('tab', { name: 'Tree' }).click();
  await page.getByRole('button', { name: /^switch-poe( |$)/ }).click();
  await expect(page.getByRole('application', { name: 'Canvas: switch-poe' })).toBeVisible();
  await page.keyboard.press('i');
  const picker = page.getByRole('dialog', { name: 'Insert a fragment' });
  await expect(picker).toBeVisible();
  return picker;
}

for (const [theme, canvas] of [
  ['Dark', 'Light canvas'],
  ['Light', 'Dark canvas'],
] as const) {
  test(`the Insert list is accessible: ${theme}, ${canvas.toLowerCase()}`, async ({ page }) => {
    await inserting(page);
    await page.getByRole('button', { name: theme, exact: true }).click();
    await page.getByRole('button', { name: canvas }).click();
    expect(await violations(page)).toEqual([]);
  });
}

test('drags a fragment from the list onto the canvas, written where it is dropped', async ({ page }, info) => {
  const picker = await inserting(page);
  const canvas = page.getByRole('application');
  const box = (await canvas.boundingBox())!;
  await picker.getByRole('button', { name: 'disk-stack' }).dragTo(canvas, { targetPosition: { x: box.width / 2, y: box.height / 2 } });
  // the new insert selected, as one thing
  await expect(page.getByRole('complementary', { name: 'Parts' }).locator('button[aria-current="true"]')).toHaveText(/^insert\s*disk-stack/);
  await page.screenshot({ path: info.outputPath('inserted.png') });
  await page.getByRole('tab', { name: 'Source' }).click();
  const source = page.getByRole('textbox', { name: /^Source of sample\.ilx/ });
  // the new one, beside the one the sample has
  await expect(source).toHaveValue(/\n {4}insert "disk-stack" cx=-?[\d.]+ cy=-?[\d.]+\n/);
  const inserts = async () => ((await source.inputValue()).match(/insert "disk-stack"/g) ?? []).length;
  expect(await inserts()).toBe(2);
  // undone in one step
  await page.getByRole('button', { name: 'Undo' }).click();
  await expect.poll(inserts).toBe(1);
});
