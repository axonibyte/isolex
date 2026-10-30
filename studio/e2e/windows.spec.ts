import { expect, test, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

// Customizable shortcuts and floating panels (#119) in a browser, over the
// studio's core: axe over Preferences and a floating panel in both themes,
// a key rebound and pressed, and a tab dragged out to float, moved, and
// dropped back on the tabs.

const violations = async (page: Page) => (await new AxeBuilder({ page }).analyze()).violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' ')).join(', ')}`);

async function opened(page: Page) {
  await page.goto('/');
  await page.getByRole('button', { name: 'Open the sample library' }).click();
  await expect(page.getByRole('application', { name: /^Canvas: / })).toBeVisible();
}

for (const [theme, canvas] of [
  ['Dark', 'Light canvas'],
  ['Light', 'Dark canvas'],
] as const) {
  test(`Preferences and a floating panel are accessible: ${theme}, ${canvas.toLowerCase()}`, async ({ page }) => {
    await opened(page);
    await page.getByRole('button', { name: theme, exact: true }).click();
    await page.getByRole('button', { name: canvas }).click();
    await page.getByRole('tab', { name: 'Preview' }).click();
    await page.getByRole('button', { name: 'Float the Preview panel' }).click();
    await expect(page.getByRole('dialog', { name: 'Preview' })).toBeVisible();
    expect(await violations(page)).toEqual([]);

    await page.getByRole('button', { name: 'Preferences', exact: true }).click();
    const prefs = page.getByRole('dialog', { name: 'Preferences' });
    await expect(prefs.getByRole('table', { name: 'Tools' })).toBeVisible();
    // one being changed, and a refusal shown
    await prefs.getByRole('button', { name: 'Shortcut for Box: B' }).click();
    await page.keyboard.press('Shift+S');
    await expect(prefs.getByRole('alert')).toHaveText('Shift+S is already Shadow: change that first');
    expect(await violations(page)).toEqual([]);
  });
}

test('a key rebound runs its command', async ({ page }) => {
  await opened(page);
  await page.keyboard.press('Control+,');
  const prefs = page.getByRole('dialog', { name: 'Preferences' });
  await prefs.getByRole('button', { name: 'Shortcut for Measure: M' }).click();
  await page.keyboard.press('Shift+M');
  await expect(prefs.getByRole('status')).toHaveText('Measure is Shift+M now');
  await prefs.getByRole('button', { name: 'Close' }).click();

  await page.keyboard.press('m');
  await expect(page.getByRole('button', { name: 'Measure', exact: true })).toHaveAttribute('aria-pressed', 'false');
  await page.keyboard.press('Shift+M');
  await expect(page.getByRole('button', { name: 'Measure', exact: true })).toHaveAttribute('aria-pressed', 'true');
  // kept: read again on reload
  await page.reload();
  await page.getByRole('button', { name: 'Open the sample library' }).click();
  await expect(page.getByRole('button', { name: 'Measure', exact: true })).toHaveAttribute('title', 'Measure (Shift+M)');
});

test('a tab dragged out floats, and dropped on the tabs docks', async ({ page }, info) => {
  await opened(page);
  const tab = page.getByRole('tab', { name: 'Source' });
  const at = (await tab.boundingBox())!;
  await page.mouse.move(at.x + at.width / 2, at.y + at.height / 2);
  await page.mouse.down();
  await page.mouse.move(500, 300, { steps: 6 });
  await page.mouse.move(520, 320, { steps: 2 });
  await page.mouse.up();
  const panel = page.getByRole('dialog', { name: 'Source' });
  await expect(panel).toBeVisible();
  await expect(page.getByRole('tab', { name: 'Source' })).toHaveCount(0);
  // the panel works floating: the source is there to edit
  await expect(panel.getByRole('textbox')).toBeVisible();
  await page.screenshot({ path: info.outputPath('floating.png') });

  // by its title bar onto the tabs
  const head = (await panel.getByRole('heading', { name: 'Source' }).boundingBox())!;
  const tabs = (await page.getByRole('tablist').boundingBox())!;
  await page.mouse.move(head.x + 10, head.y + head.height / 2);
  await page.mouse.down();
  await page.mouse.move(tabs.x + tabs.width - 20, tabs.y + tabs.height / 2, { steps: 8 });
  await page.mouse.up();
  await expect(panel).toHaveCount(0);
  await expect(page.getByRole('tab', { name: 'Source' })).toHaveAttribute('aria-selected', 'true');
});
