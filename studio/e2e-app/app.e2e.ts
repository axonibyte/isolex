// The built app, end to end (#73): opened on a copy of the sample library
// from its command line, a box drawn with the box tool and a path with
// the pen, another icon opened from the Tree, the library saved, and the
// folder then checked by the compiler, which must find nothing wrong.

import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { $, browser, expect } from '@wdio/globals';

const library = process.env.STUDIO_E2E_LIBRARY!;
const root = resolve(fileURLToPath(new URL('../..', import.meta.url)));

const S = 40;
const COS = 0.8660254037844387;

/** Where the canvas is on the page, and its camera: its offset and zoom. */
async function camera(): Promise<{ left: number; top: number; tx: number; ty: number; k: number }> {
  return browser.execute(() => {
    const app = document.querySelector('[role=application]')!;
    const r = app.getBoundingClientRect();
    const t = app.querySelector('svg > g')!.getAttribute('transform')!;
    const [tx, ty, k] = t.match(/-?[\d.]+(e-?\d+)?/g)!.map(Number);
    return { left: r.left, top: r.top, tx: tx!, ty: ty!, k: k! };
  });
}

/** A tile point on the page. */
async function tile(x: number, y: number, z: number): Promise<[number, number]> {
  const c = await camera();
  const vx = (x - y) * COS * S;
  const vy = (x + y) * 0.5 * S - z * S;
  return [Math.round(c.left + c.tx + vx * c.k), Math.round(c.top + c.ty + vy * c.k)];
}

/** A flat point, in output units, on the page. */
async function flat(x: number, y: number): Promise<[number, number]> {
  const c = await camera();
  return [Math.round(c.left + c.tx + x * c.k), Math.round(c.top + c.ty + y * c.k)];
}

const mouse = () => browser.action('pointer', { parameters: { pointerType: 'mouse' } });

async function click([x, y]: [number, number]) {
  await mouse().move({ x, y }).down().up().perform();
}

async function drag([x0, y0]: [number, number], [x1, y1]: [number, number]) {
  await mouse().move({ x: x0, y: y0 }).down().move({ x: x1, y: y1, duration: 250 }).up().perform();
}

const sourceText = () => browser.execute(() => (document.querySelector('textarea') as HTMLTextAreaElement | null)?.value ?? '');

describe('isolex studio, built', () => {
  it('opens a library, draws, opens another icon, saves, and the folder checks', async () => {
    // opened from the command line: an icon drawn, whichever compiled first
    const canvas = $('[role=application]');
    await canvas.waitForExist({ timeout: 60_000 });
    await browser.waitUntil(async () => ((await canvas.getAttribute('aria-label')) ?? '').startsWith('Canvas: '), { timeout: 60_000 });

    // switch-l2, from the Tree, its source beside it
    await $('button[role=tab]=Tree').click();
    await $('[aria-label="Icons"] [data-name="switch-l2"]').click();
    await browser.waitUntil(async () => (await canvas.getAttribute('aria-label')) === 'Canvas: switch-l2', { timeout: 30_000 });
    await $('button[role=tab]=Source').click();
    await browser.waitUntil(async () => (await sourceText()).includes('icon "switch-l2"'), { timeout: 30_000 });

    // a box: its footprint dragged on the floor, then its height, clicked
    await browser.keys(['b']);
    await drag(await tile(0, 2, 0), await tile(1, 3, 0));
    const [hx, hy] = await tile(1, 3, 0);
    const k = (await camera()).k;
    await mouse()
      .move({ x: hx, y: Math.round(hy - 20 * k) })
      .perform();
    await click([hx, Math.round(hy - 20 * k)]);
    await browser.waitUntil(async () => (await sourceText()).includes('box 0 2 0 1 1 0.5'), { timeout: 30_000, timeoutMsg: 'the box was not written' });

    // a path with the pen, left open with Enter
    await browser.keys(['p']);
    await click(await flat(-30, 40));
    await click(await flat(-10, 40));
    await click(await flat(-10, 52));
    await browser.keys(['Enter']);
    await browser.waitUntil(async () => (await sourceText()).includes('path d="M-30,40 L-10,40 L-10,52"'), { timeout: 30_000, timeoutMsg: 'the path was not written' });

    // another icon, from the Tree
    await $('button[role=tab]=Tree').click();
    await $('[aria-label="Icons"] [data-name="switch-poe"]').click();
    await browser.waitUntil(async () => (await canvas.getAttribute('aria-label')) === 'Canvas: switch-poe', { timeout: 30_000 });

    // saved: every changed file written
    await browser.keys(['Control', 's']);
    await browser.keys(['Control']);
    await browser.waitUntil(async () => (await $('footer').getText()).includes('saved') && !(await $('footer').getText()).includes('unsaved'), { timeout: 30_000, timeoutMsg: 'the library was not saved' });

    const written = readFileSync(join(library, 'sample.ilx'), 'utf8');
    expect(written).toContain('box 0 2 0 1 1 0.5');
    expect(written).toContain('path d="M-30,40 L-10,40 L-10,52"');

    // and the compiler finds nothing wrong with the folder (the sample's
    // one warning is not wrong)
    const check = spawnSync('cargo', ['run', '--release', '--locked', '--quiet', '--bin', 'isolex', '--manifest-path', join(root, 'Cargo.toml'), '--', 'check', library], { encoding: 'utf8', shell: process.platform === 'win32' });
    expect(check.stdout).toContain('ok: 10 definition(s), 7 icon(s) emitted');
    expect(check.status).toBe(0);
  });
});
