import { expect, test, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

// The pen and the node tool (#83) in a browser, over the studio's core:
// draw a teardrop in a flat icon, reshape it, and undo it all.

async function editing(page: Page, name: string) {
  await page.goto('/');
  await page.getByRole('button', { name: 'Open the sample library' }).click();
  await page.getByRole('tab', { name: 'Tree' }).click();
  await page.getByRole('list', { name: 'Icons' }).getByRole('button', { name, exact: true }).click();
  await expect(page.getByRole('application', { name: `Canvas: ${name}` })).toBeVisible();
  await page.getByRole('tab', { name: 'Source' }).click();
}

/** Where a flat point, in output units, is on the page, by the canvas'
    camera now. */
async function at(page: Page, x: number, y: number): Promise<[number, number]> {
  const box = (await page.getByRole('application').boundingBox())!;
  const t = (await page.locator('[role=application] svg > g').getAttribute('transform'))!;
  const [tx, ty, k] = t.match(/-?[\d.]+(e-?\d+)?/g)!.map(Number) as [number, number, number];
  return [box.x + tx + x * k, box.y + ty + y * k];
}

const source = (page: Page) => page.getByRole('textbox', { name: /^Source of sample\.ilx/ });

async function click(page: Page, x: number, y: number) {
  await page.mouse.move(...(await at(page, x, y)));
  await page.mouse.down();
  await page.mouse.up();
}

async function drag(page: Page, from: [number, number], to: [number, number]) {
  await page.mouse.move(...(await at(page, ...from)));
  await page.mouse.down();
  await page.mouse.move(...(await at(page, ...to)), { steps: 5 });
  await page.mouse.up();
}

test('a teardrop drawn with the pen, reshaped with the node tool, and undone', async ({ page }) => {
  await editing(page, 'state-planned');
  const before = await source(page).inputValue();

  // a corner at the top, two smooth points dragged out, closed on the first
  await page.keyboard.press('p');
  await click(page, 12, 2);
  await drag(page, [18, 14], [18, 18]);
  await drag(page, [6, 14], [6, 10]);
  await expect(page.locator('[role=application] .dot')).toHaveCount(3);
  await click(page, 12, 2);
  await expect(source(page)).toHaveValue(/path d="M12,2 C12,2 18,10 18,14 C18,18 6,18 6,14 C6,10 12,2 12,2 Z"/);
  expect((await new AxeBuilder({ page }).analyze()).violations.map((v) => v.id)).toEqual([]);

  // the node tool: the tip pulled up, a smooth point's handle turned
  await page.keyboard.press('a');
  await expect(page.locator('[data-handle="pt-0"]')).toBeVisible();
  await drag(page, [12, 2], [12, 1]);
  await expect(source(page)).toHaveValue(/path d="M12,1 C12,1 18,10 18,14 C18,18 6,18 6,14 C6,10 12,1 12,1 Z"/);
  await drag(page, [18, 18], [22, 14]);
  // the other handle of the smooth point turns with it, as long as it was
  await expect(source(page)).toHaveValue(/path d="M12,1 C12,1 14,14 18,14 C22,14 6,18 6,14 C6,10 12,1 12,1 Z"/);
  expect((await new AxeBuilder({ page }).analyze()).violations.map((v) => v.id)).toEqual([]);

  // three steps, three undos
  for (let i = 0; i < 3; i++) await page.getByRole('button', { name: 'Undo' }).click();
  await expect(source(page)).toHaveValue(before);
});
