// The drawing tools (#83) in the window, over the studio's core: each
// tool's gesture, the node it writes, and that the source then compiles
// and draws it; the node tool shaping a path and a polygon; the Design
// tab's paint and repeat. In jsdom the canvas is never measured, so its
// camera starts at scale 1 with the viewBox's origin at (400, 300); flat
// work is done zoomed in about that origin, as it would be on screen,
// where a snap pulls from a few units, not from across the icon.

import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import App from './App.svelte';
import { core } from './lib/core';
import { apply, type Point, type Point3 } from './lib/geometry';
import { PROJECTION } from './lib/tools.svelte';
import { at as along, parse, sides } from './lib/path';
import sampleStudio from '../../../tests/fixtures/sample.studio.golden.json';

const sources = sampleStudio.sources as Record<string, { kind: string }[]>;

afterEach(() => {
  cleanup();
  localStorage.clear();
});

/** Where a tile point is on screen. */
const at = (p: Point3) => {
  const [x, y] = apply(PROJECTION, p);
  return { clientX: 400 + x, clientY: 300 + y };
};
/** The zoom flat work is done at, and where a flat point (output units)
    is on screen at it. */
const K = 20;
const flat = ([x, y]: Point) => ({ clientX: 400 + x * K, clientY: 300 + y * K });
/** The canvas zoomed to `K` about the viewBox's origin. */
const zoomed = () => fireEvent.wheel(canvas(), { deltaY: -Math.log(K) / 0.0025, clientX: 400, clientY: 300 });

async function editing(name: string) {
  const backend = await core();
  const ask = vi.spyOn(backend, 'ask');
  render(App, { backend, sample: true });
  await fireEvent.click(screen.getByRole('button', { name: 'Open the sample library' }));
  await screen.findByRole('application', { name: /^Canvas: / });
  await fireEvent.click(screen.getByRole('tab', { name: 'Tree' }));
  await fireEvent.click(within(screen.getByRole('list', { name: 'Icons' })).getByRole('button', { name: new RegExp(`^${name}$`) }));
  await screen.findByRole('application', { name: `Canvas: ${name}` });
  const parts = screen.getByRole('complementary', { name: 'Parts' });
  await waitFor(() => expect(within(parts).queryByText('Reading the definition…')).toBeNull());
  const text = async () => (await backend.source('sample.ilx'))!.text;
  return { backend, ask, parts, text };
}

const canvas = () => screen.getByRole('application');
const errors = () => /\d+ errors?/.test(screen.getByRole('contentinfo').textContent ?? '');
const shape = (i: number) => canvas().querySelector(`[data-shape="${i}"]`)!;
const first = (icon: string, kind: string) => shape(sources[icon]!.findIndex((o) => o.kind === kind));
const tool = async (key: string, shiftKey = false) => fireEvent.keyDown(window, { key, shiftKey });
const there = (find: () => Element | null) =>
  waitFor(() => {
    const el = find();
    if (!el) throw new Error('not there yet');
    return el;
  });

async function click(where: { clientX: number; clientY: number }, on: Element = canvas()) {
  await fireEvent.pointerDown(on, { button: 0, ...where });
  await fireEvent.pointerUp(canvas(), { button: 0, ...where });
}
async function drag(path: { clientX: number; clientY: number }[], on: Element = canvas(), opts: { shiftKey?: boolean; altKey?: boolean } = {}) {
  await fireEvent.pointerDown(on, { button: 0, ...path[0]!, ...opts });
  for (const p of path.slice(1)) await fireEvent.pointerMove(canvas(), { button: 0, buttons: 1, ...p, ...opts });
  await fireEvent.pointerUp(canvas(), { button: 0, ...path.at(-1)!, ...opts });
}
/** A move with no button held. */
const hover = (where: { clientX: number; clientY: number }, on: Element = canvas()) => fireEvent.pointerMove(on, { button: 0, ...where });

describe('the 3-D tools', () => {
  it('draws a cylinder: the radius, then the height', async () => {
    const { text } = await editing('switch-poe');
    await tool('c');
    await drag([at([0, 2, 0]), at([0.5, 2, 0])]);
    const up = at([0.5, 2, 0]);
    await hover({ clientX: up.clientX, clientY: up.clientY - 20 });
    expect(await screen.findByText('r 0.5 · h 0.5')).toBeTruthy();
    await click({ clientX: up.clientX, clientY: up.clientY - 20 });
    await waitFor(async () => expect(await text()).toContain('    cylinder 0 2 0 0.5 0.5\n'));
  });

  it('draws a face rect on the side of a box pressed on', async () => {
    const { text } = await editing('switch-l3');
    await tool('f');
    // the chassis' front, its first face
    const front = first('switch-l3', 'box');
    await hover(at([0.75, 1, 0.1]), front);
    expect(canvas().querySelector('.face')).not.toBeNull();
    await drag([at([0.5, 1, 0.02]), at([1, 1, 0.25])], front);
    await waitFor(async () => expect(await text()).toContain('facerect "front" 0.5 1 0 0.5 0.25'));
  });

  it('draws a shadow from its center', async () => {
    const { text } = await editing('switch-poe');
    await tool('S', true);
    const c = at([1, 2, 0]);
    await drag([c, { clientX: c.clientX + PROJECTION[0] * 0.5, clientY: c.clientY + PROJECTION[1] * 0.25 }]);
    await waitFor(async () => expect(await text()).toContain('shadow 1 2 0.5 0.25'));
  });

  it('draws a segment by two clicks', async () => {
    const { text, ask } = await editing('switch-poe');
    await tool('L', true);
    await click(at([0, 2, 0]));
    expect(ask).not.toHaveBeenCalled();
    await hover(at([1, 3, 0]));
    expect(canvas().querySelectorAll('.preview')).toHaveLength(1);
    await click(at([1, 3, 0]));
    await waitFor(async () => expect(await text()).toContain('seg 0 2 0 1 3 0'));
  });

  it('draws a 3-D polygon, closed by Enter', async () => {
    const { text } = await editing('switch-poe');
    await tool('G', true);
    for (const p of [
      [0, 2, 0],
      [1, 2, 0],
      [1, 3, 0],
    ] as Point3[])
      await click(at(p));
    await fireEvent.keyDown(window, { key: 'Enter' });
    await waitFor(async () => expect(await text()).toContain('poly3d {\n      pt 0 2 0\n      pt 1 2 0\n      pt 1 3 0\n    }'));
  });
});

describe('the flat tools', () => {
  it('draws on one anchor at the origin, shared by the shapes after it', async () => {
    const { text } = await editing('switch-poe');
    await zoomed();
    await tool('l');
    await drag([flat([0, 0]), flat([10, 0.3])]);
    await waitFor(async () => expect(await text()).toContain('    at 0 0 0 {\n      line 0 0 10 0\n    }\n'));
    await tool('r');
    await drag([flat([4, 4]), flat([-2, 1])]);
    await waitFor(async () => expect(await text()).toContain('    at 0 0 0 {\n      line 0 0 10 0\n      rect -2 1 6 3\n    }\n'));
  });

  it('keeps 15° steps for a line, and a circle for an ellipse, with Shift', async () => {
    const { text } = await editing('state-planned');
    await zoomed();
    await tool('l');
    await drag([flat([2, 2]), flat([12, 3])], canvas(), { shiftKey: true });
    await waitFor(async () => expect(await text()).toContain('line 2 2 12 2\n'));
    await tool('e');
    await drag([flat([4, 4]), flat([12, 10])], canvas(), { shiftKey: true });
    await waitFor(async () => expect(await text()).toContain('circle 8 8 4\n'));
  });

  it('draws an arc: the radius from the center, then the sweep', async () => {
    const { text } = await editing('state-planned');
    await zoomed();
    await tool('A', true);
    await drag([flat([12, 12]), flat([18, 12])]);
    await hover(flat([12, 6]));
    expect(await screen.findByText('0° to 90°')).toBeTruthy();
    await click(flat([12, 6]));
    await waitFor(async () => expect(await text()).toContain('arc 12 12 6 0 90\n'));
  });

  it('draws a polygon, closed on its first point; Esc cancels one', async () => {
    const { text, ask } = await editing('state-planned');
    await zoomed();
    await tool('g');
    await click(flat([2, 2]));
    await click(flat([8, 2]));
    await fireEvent.keyDown(window, { key: 'Escape' });
    // one point is not a polygon: Enter keeps drawing it
    await click(flat([5, 9]));
    await fireEvent.keyDown(window, { key: 'Enter' });
    await fireEvent.keyDown(window, { key: 'Escape' });
    expect(ask).not.toHaveBeenCalled();
    for (const p of [
      [2, 2],
      [8, 2],
      [5, 9],
      [2, 2],
    ] as Point[])
      await click(flat(p));
    await waitFor(async () => expect(await text()).toContain('poly {\n        pt 2 2\n        pt 8 2\n        pt 5 9\n      }'));
  });

  it('draws a teardrop with the pen: a corner, two smooth points, closed', async () => {
    const { text } = await editing('state-planned');
    await zoomed();
    await tool('p');
    await click(flat([12, 2]));
    // dragged: the handles, mirrored
    await drag([flat([18, 14]), flat([18, 18])]);
    await drag([flat([6, 14]), flat([6, 10])]);
    expect(canvas().querySelectorAll('.dot')).toHaveLength(3);
    expect(canvas().querySelectorAll('.dot.first')).toHaveLength(1);
    await click(flat([12, 2]));
    await waitFor(async () => expect(await text()).toContain('path d="M12,2 C12,2 18,10 18,14 C18,18 6,18 6,14 C6,10 12,2 12,2 Z"'));
  });

  it('leaves a pen path open with Enter', async () => {
    const { text } = await editing('state-planned');
    await zoomed();
    await tool('p');
    await click(flat([2, 20]));
    await click(flat([10, 20]));
    await fireEvent.keyDown(window, { key: 'Enter' });
    await waitFor(async () => expect(await text()).toContain('path d="M2,20 L10,20"'));
  });
});

describe('shaping flat shapes', () => {
  it('moves and resizes a rectangle with the select tool', async () => {
    const { text } = await editing('state-planned');
    await zoomed();
    const r = first('state-planned', 'rect');
    await drag([flat([4, 4]), flat([6, 5])], r);
    await waitFor(async () => expect(await text()).toContain('rect 5 4 18 18 rx=3'));
    const corner = await there(() => canvas().querySelector('[data-handle="rect-2"]'));
    await drag([flat([23, 22]), flat([20, 20])], corner);
    await waitFor(async () => expect(await text()).toContain('rect 5 4 15 16 rx=3'));
  });

  it('moves a polygon’s point with the node tool, and refuses one of expressions', async () => {
    const { text } = await editing('state-down');
    await zoomed();
    await tool('a');
    await click(flat([12, 12]), first('state-down', 'poly'));
    const pt = await there(() => canvas().querySelector('[data-handle="pt-1"]'));
    await drag([flat([17, 2]), flat([18, 1])], pt);
    await waitFor(async () => expect(await text()).toContain('        pt 7 2\n        pt 18 1\n        pt 22 7\n'));
    // Delete takes the point touched last
    await fireEvent.keyDown(window, { key: 'Delete' });
    await waitFor(async () => expect(await text()).toContain('        pt 7 2\n        pt 22 7\n'));
  });

  it('shapes a path: a point, a smooth point by Alt+click, a point added and removed', async () => {
    const { text } = await editing('state-planned');
    await zoomed();
    await tool('p');
    await click(flat([2, 2]));
    await click(flat([12, 2]));
    await click(flat([12, 12]));
    await fireEvent.keyDown(window, { key: 'Enter' });
    await waitFor(async () => expect(await text()).toContain('path d="M2,2 L12,2 L12,12"'));
    await tool('a');
    // the path is selected as drawn: its points have handles
    const p1 = await there(() => canvas().querySelector('[data-handle="pt-1"]'));
    await drag([flat([12, 2]), flat([14, 2])], p1);
    await waitFor(async () => expect(await text()).toContain('path d="M2,2 L14,2 L12,12"'));
    // Alt+click: smooth, its handles along the line between its neighbors
    const again = await there(() => canvas().querySelector('[data-handle="pt-1"]'));
    await fireEvent.pointerDown(again, { button: 0, ...flat([14, 2]), altKey: true });
    await fireEvent.pointerUp(canvas(), { button: 0, ...flat([14, 2]), altKey: true });
    await waitFor(async () => expect(await text()).toMatch(/path d="M2,2 C2,2 [\d.,-]+ 14,2 C[\d.,-]+ 12,12 12,12"/));
    expect(canvas().querySelectorAll('[data-handle^="in-"], [data-handle^="out-"]').length).toBeGreaterThan(0);
    // a double-click on the first side, halfway along its curve, adds a
    // point there
    const d = (await text()).match(/path d="(M2,2 [^"]+)"/)![1]!;
    await fireEvent.dblClick(canvas(), flat(along(sides(parse(d)!)[0]!, 0.5)));
    await waitFor(() => expect(canvas().querySelector('[data-handle="pt-3"]')).not.toBeNull());
    await fireEvent.keyDown(window, { key: 'Delete' });
    await waitFor(() => expect(canvas().querySelector('[data-handle="pt-3"]')).toBeNull());
  });
});

describe('the Design tab', () => {
  it('paints with the lexicon’s tokens and widths only', async () => {
    const { text } = await editing('state-planned');
    await zoomed();
    await click(flat([4, 4]), first('state-planned', 'rect'));
    await fireEvent.click(screen.getByRole('tab', { name: 'Design' }));
    const fill = await screen.findByRole('group', { name: 'Fill' });
    // the sample lexicon's eight tokens, and none
    expect(within(fill).getAllByRole('button')).toHaveLength(9);
    await fireEvent.click(within(fill).getByRole('button', { name: 'Fill: accent' }));
    await waitFor(async () => expect(await text()).toContain('rect 3 3 18 18 rx=3 stroke="ink-muted" width=1.5 dash="3 2.5" fill=accent'));
    const width = screen.getByRole('combobox', { name: 'Width' });
    expect([...width.querySelectorAll('option')].map((o) => o.textContent)).toEqual(['1.2', '1.4', '1.5', '2.2']);
    await fireEvent.change(width, { target: { value: '2.2' } });
    await waitFor(async () => expect(await text()).toContain('width=2.2'));
    const radius = screen.getByRole('spinbutton', { name: 'Radius' });
    await fireEvent.input(radius, { target: { value: '0' } });
    await fireEvent.change(radius);
    await waitFor(async () => expect(await text()).not.toContain('rx=3'));
  });

  it('repeats a shape along an axis as one repeat, then counts its copies', async () => {
    const { text } = await editing('switch-poe');
    await click(at([1, 0.5, 0]), first('switch-poe', 'tile'));
    await fireEvent.click(screen.getByRole('tab', { name: 'Design' }));
    const copies = await screen.findByRole('spinbutton', { name: 'Copies' });
    await fireEvent.input(copies, { target: { value: '3' } });
    await fireEvent.change(screen.getByRole('combobox', { name: 'Along' }), { target: { value: 'z' } });
    await fireEvent.input(screen.getByRole('spinbutton', { name: 'Step' }), { target: { value: '0.5' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Repeat' }));
    await waitFor(async () => expect(await text()).toContain('    repeat 3 "i" {\n      tile 0 0 "0 + i * 0.5" 2 1 fill="body-top" dash="3 2.5"\n    }\n'));
    // it compiles, and draws three tiles
    await waitFor(() => expect(errors()).toBe(false));
  });
});
