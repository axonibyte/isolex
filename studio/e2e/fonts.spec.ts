import { expect, test, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

// Fonts in the editor (#84) in a browser, over the studio's core and the
// compiler's text fixture: open a font, draw in a glyph with the pen, and
// see the text an icon sets in it change.

const violations = async (page: Page) => (await new AxeBuilder({ page }).analyze()).violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target).join(', ')}`);

/** A point of the plain font's em grid on the page: the glyph shown an em
    of 10 set 200 units tall, its origin a quarter em in, below an ascent
    of 7. */
async function em(page: Page, x: number, y: number): Promise<[number, number]> {
  const box = (await page.getByRole('application').boundingBox())!;
  const t = (await page.locator('[role=application] svg > g').getAttribute('transform'))!;
  const [tx, ty, k] = t.match(/-?[\d.]+(e-?\d+)?/g)!.map(Number) as [number, number, number];
  const vx = (2.5 + x) * 20;
  const vy = (2.5 + 7 - y) * 20;
  return [box.x + tx + vx * k, box.y + ty + vy * k];
}

async function click(page: Page, x: number, y: number) {
  await page.mouse.move(...(await em(page, x, y)));
  await page.mouse.down();
  await page.mouse.up();
}

test('a glyph drawn with the pen changes the text an icon sets in its font', async ({ page }) => {
  await page.goto('/?library=text');
  await page.getByRole('button', { name: 'Open the sample library' }).click();
  await page.getByRole('tab', { name: 'Tree' }).click();
  const icons = page.getByRole('list', { name: 'Icons' });
  const shaping = icons.locator('[data-name="shaping"] svg');
  await expect(shaping).toBeVisible();
  const before = await shaping.innerHTML();

  // the font, its glyphs set out
  await page.getByRole('list', { name: 'Fonts' }).locator('[data-name="plain"]').click();
  await expect(page.getByRole('application', { name: 'Canvas: plain' })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Glyphs' })).toBeVisible();
  expect(await violations(page)).toEqual([]);

  // its o, which "To" sets, with a stroke added by the pen
  const glyphs = page.getByRole('complementary', { name: 'Parts' });
  await glyphs.getByRole('button', { name: 'o', exact: true }).click();
  await expect(page.getByRole('application', { name: 'Canvas: plain, glyph o' })).toBeVisible();
  // fitted to the window, its frame wholly on the canvas
  await expect(async () => {
    const box = (await page.getByRole('application').boundingBox())!;
    const frame = (await page.locator('[role=application] .frame').boundingBox())!;
    expect(frame.y).toBeGreaterThanOrEqual(box.y);
    expect(frame.y + frame.height).toBeLessThanOrEqual(box.y + box.height);
  }).toPass();
  await page.getByRole('tab', { name: 'Design' }).click();
  await expect(page.getByRole('group', { name: 'Kern T o' })).toBeVisible();
  expect(await violations(page)).toEqual([]);
  await page.keyboard.press('p');
  await click(page, 0, 6);
  await click(page, 4, 6);
  await page.keyboard.press('Enter');
  await page.getByRole('tab', { name: 'Source' }).click();
  await expect(page.getByRole('textbox', { name: /^Source of text\.ilx/ })).toHaveValue(/glyph "o" advance=5\.5 \{[^}]*path d="M0,6 L4,6"/);

  // the icon that sets "To" is drawn again, its o with the stroke
  await page.getByRole('tab', { name: 'Tree' }).click();
  await expect(icons.locator('[data-name="shaping"] .redrawn')).toBeVisible();
  expect(await shaping.innerHTML()).not.toBe(before);
});
