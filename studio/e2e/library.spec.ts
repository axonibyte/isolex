import { expect, test, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

// Managing a library (#72) in a browser, over the studio's core: the
// Library and Lexicon Properties, each accessible, and a change made on
// disk with and without unsaved edits (the core outside the app keeps its
// disk in memory; `studioDisk` changes a file there, as another program
// would).

const violations = async (page: Page) => (await new AxeBuilder({ page }).analyze()).violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target).join(', ')}`);
const source = (page: Page) => page.getByRole('textbox', { name: /^Source of sample\.ilx/ });

async function opened(page: Page) {
  await page.goto('/');
  await page.getByRole('button', { name: 'Open the sample library' }).click();
  await expect(page.getByRole('application', { name: /^Canvas: / })).toBeVisible();
}

/** A file changed on disk, to `text`. */
const onDisk = (page: Page, text: string) =>
  page.evaluate(async (t) => {
    const w = window as unknown as { studioDisk: (file: string, text: string | null) => Promise<void> };
    await w.studioDisk('sample.ilx', t);
  }, text);

test('the Library and Lexicon Properties are accessible, and rename and color', async ({ page }) => {
  await opened(page);
  await page.getByRole('button', { name: 'Library', exact: true }).click();
  const lib = page.getByRole('dialog', { name: 'Library' });
  await expect(lib.getByRole('list', { name: 'Definitions in sample.ilx' })).toBeVisible();
  expect(await violations(page)).toEqual([]);
  await lib.getByRole('button', { name: 'Rename state-down' }).click();
  await lib.getByRole('textbox', { name: 'New name for state-down' }).fill('state-alarm');
  await lib.getByRole('textbox', { name: 'New name for state-down' }).press('Enter');
  await expect(lib.getByRole('status')).toHaveText('Renamed state-down to state-alarm, and every reference to it');
  await page.keyboard.press('Escape');

  await page.getByRole('button', { name: 'Lexicon', exact: true }).click();
  const lex = page.getByRole('dialog', { name: 'Lexicon Properties' });
  await expect(lex.getByRole('list', { name: 'Tokens' })).toBeVisible();
  expect(await violations(page)).toEqual([]);
  await lex.getByRole('tab', { name: 'Swatches' }).click();
  await lex.getByLabel('accent on light').fill('#cc2222');
  await expect(lex.getByText('#cc2222')).toBeVisible();
  expect(await violations(page)).toEqual([]);
  await page.keyboard.press('Escape');
  await expect(source(page)).toHaveValue(/palette \{\n {2}accent light="#cc2222" dark="#ef8a8a"\n\}/);
  await expect(source(page)).toHaveValue(/badge "state-alarm"/);
});

test('a change on disk loads with nothing unsaved, and with unsaved edits asks', async ({ page }) => {
  await opened(page);
  await expect(source(page)).toHaveValue(/param ports 12/);
  const saved = await source(page).inputValue();
  // nothing unsaved: loaded, as if edited here
  await onDisk(page, saved.replace('param ports 12', 'param ports 6'));
  await expect(source(page)).toHaveValue(/param ports 6/);
  await expect(page.getByRole('alert')).toHaveCount(0);

  // an edit unsaved, then a change on disk: the strip, and the studio's copy kept
  await source(page).click();
  await source(page).press('Control+Home');
  await source(page).pressSequentially('// mine\n');
  await expect(source(page)).toHaveValue(/^\/\/ mine\n/);
  await onDisk(page, saved.replace('param ports 12', 'param ports 4'));
  const strip = page.getByRole('alert');
  await expect(strip).toContainText('The library changed on disk (sample.ilx), and you have unsaved changes.');
  expect(await violations(page)).toEqual([]);
  await expect(source(page)).toHaveValue(/^\/\/ mine\n[\s\S]*param ports 6/);
  await strip.getByRole('button', { name: 'Reload from disk' }).click();
  await strip.getByRole('button', { name: 'Discard and reload' }).click();
  await expect(page.getByRole('alert')).toHaveCount(0);
  await expect(source(page)).toHaveValue(/param ports 4/);
  await expect(source(page)).not.toHaveValue(/\/\/ mine/);
});
