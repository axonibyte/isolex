import { readFile } from 'node:fs/promises';
import { expect, test, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

// Export (#112) in a browser, over the studio's core: PNG is drawn by
// the browser from the core's SVG, so it is checked here, where there is
// one, file by file; the dialog checked with axe.

const violations = async (page: Page) => (await new AxeBuilder({ page }).analyze()).violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target).join(', ')}`);

/** A PNG's width and height, from its IHDR chunk. */
function pngSize(png: Buffer): { width: number; height: number } {
  expect([...png.subarray(0, 8)]).toEqual([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  expect(png.subarray(12, 16).toString('latin1')).toBe('IHDR');
  return { width: png.readUInt32BE(16), height: png.readUInt32BE(20) };
}

test('the open icon as PNG, at two sizes on both grounds, each drawn at its size', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Open the sample library' }).click();
  const canvas = page.getByRole('application', { name: /^Canvas: / });
  await expect(canvas).toBeVisible();
  const icon = (await canvas.getAttribute('aria-label'))!.replace('Canvas: ', '');

  await page.getByRole('button', { name: 'Export…' }).click();
  const dialog = page.getByRole('dialog', { name: 'Export' });
  await dialog.getByRole('combobox', { name: 'As' }).selectOption('png');
  await dialog.getByRole('checkbox', { name: 'Dark' }).check();
  const sizes = dialog.getByRole('group', { name: 'Sizes' });
  await sizes.getByRole('checkbox', { name: '32', exact: true }).uncheck();
  await sizes.getByRole('checkbox', { name: '16', exact: true }).check();
  await sizes.getByRole('checkbox', { name: '128', exact: true }).check();
  await expect(dialog.getByText('4 files, downloaded.')).toBeVisible();
  expect(await violations(page)).toEqual([]);

  const files: Record<string, Buffer> = {};
  page.on('download', async (d) => {
    files[d.suggestedFilename()] = await readFile((await d.path())!);
  });
  await dialog.getByRole('button', { name: 'Export' }).click();
  await expect(dialog.getByRole('status')).toHaveText('Exported 4 files');
  await expect.poll(() => Object.keys(files).sort()).toEqual([`${icon}@128-dark.png`, `${icon}@128.png`, `${icon}@16-dark.png`, `${icon}@16.png`].sort());

  // each at its size, its longer side the size asked for
  for (const px of [16, 128]) {
    const { width, height } = pngSize(files[`${icon}@${px}.png`]);
    expect(Math.max(width, height)).toBe(px);
    expect(pngSize(files[`${icon}@${px}-dark.png`])).toEqual({ width, height });
  }
  // the two grounds are drawn in different colors
  expect(files[`${icon}@128.png`].equals(files[`${icon}@128-dark.png`])).toBe(false);
  expect(await violations(page)).toEqual([]);
});
