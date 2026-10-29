import { expect, test, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

// The Design and Preview tabs, Undo History and Problems (#70) in a
// browser, over the studio's core, each checked with axe as it is used.

async function editing(page: Page, name: string) {
  await page.goto('/');
  await page.getByRole('button', { name: 'Open the sample library' }).click();
  await page.getByRole('tab', { name: 'Tree' }).click();
  await page.getByRole('list', { name: 'Icons' }).getByRole('button', { name, exact: true }).click();
  await expect(page.getByRole('application', { name: `Canvas: ${name}` })).toBeVisible();
}

const violations = async (page: Page) => (await new AxeBuilder({ page }).analyze()).violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target).join(', ')}`);
const source = (page: Page) => page.getByRole('textbox', { name: /^Source of sample\.ilx/ });

test('params previewed and set, a shape’s numbers set, and the history gone back through', async ({ page }) => {
  await editing(page, 'switch-l3');
  await page.getByRole('tab', { name: 'Design' }).click();
  const ports = page.getByRole('group', { name: 'Param ports' });
  await expect(ports.getByText('from switch')).toBeVisible();
  expect(await violations(page)).toEqual([]);

  // previewed: four ports drawn, nothing written
  const shapes = page.locator('[role=application] [data-shape]');
  const before = await shapes.count();
  await ports.getByRole('slider', { name: 'Preview ports' }).fill('4');
  await expect(shapes).toHaveCount(before - 8);
  await expect(page.getByText(/Previewing ports = 4/)).toBeVisible();
  expect(await violations(page)).toEqual([]);
  await ports.getByRole('button', { name: 'Set as default' }).click();
  const choice = page.getByRole('dialog', { name: 'This edit needs a choice' });
  await choice.getByRole('button', { name: /^Set it in switch-l3/ }).click();
  await page.getByRole('tab', { name: 'Source' }).click();
  await expect(source(page)).toHaveValue(/ {2}param ports 4\n {2}part "routing-badge"/);

  // the history: back to before it
  await page.getByRole('button', { name: 'History' }).click();
  const history = page.getByRole('dialog', { name: 'Undo history' });
  await expect(history.getByRole('button', { name: 'Declare ports in switch-l3' })).toBeVisible();
  expect(await violations(page)).toEqual([]);
  await history.getByRole('button', { name: 'Opened' }).click();
  await expect(source(page)).not.toHaveValue(/param ports 4/);
});

test('the Preview tab and the problems list are accessible, and a problem opens its place', async ({ page }) => {
  await editing(page, 'switch-poe');
  await page.getByRole('tab', { name: 'Preview' }).click();
  await expect(page.getByRole('img', { name: 'switch-poe at 128 px' })).toHaveCount(2);
  expect(await violations(page)).toEqual([]);

  await page.getByRole('button', { name: /1 warning/ }).click();
  const problems = page.getByRole('dialog', { name: 'Problems' });
  await expect(problems.getByRole('button')).toHaveCount(1);
  expect(await violations(page)).toEqual([]);
  await problems.getByRole('button').click();
  await expect(page.getByRole('tab', { name: 'Source' })).toHaveAttribute('aria-selected', 'true');
  const line = await source(page).evaluate((a: HTMLTextAreaElement) => a.value.slice(0, a.selectionStart).split('\n').length);
  expect(line).toBe(82);
});
