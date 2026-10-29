// The Select, Box and Tile tools (#69) in the window, over the studio's
// core: each gesture's asks, and what they write. In jsdom the canvas is
// never measured, so its camera stays at scale 1 with the viewBox's
// origin at (400, 300): a tile point is on screen where `at` puts it.

import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import App from './App.svelte';
import { core } from './lib/core';
import type { Backend } from './lib/backend';
import { apply, type Point3 } from './lib/geometry';
import { PROJECTION } from './lib/tools.svelte';
import source from '../../../tests/fixtures/sample.ilx?raw';
import sampleStudio from '../../../tests/fixtures/sample.studio.golden.json';

const sources = sampleStudio.sources as Record<string, { kind: string }[]>;
/** The first shape of the icon drawn by a node of `kind`. */
const first = (icon: string, kind: string) => shape(sources[icon]!.findIndex((o) => o.kind === kind));
/** An element, once it is there. */
const there = (find: () => Element | null) =>
  waitFor(() => {
    const el = find();
    if (!el) throw new Error('not there yet');
    return el;
  });

afterEach(() => {
  cleanup();
  localStorage.clear();
});

const at = (p: Point3) => {
  const [x, y] = apply(PROJECTION, p);
  return { clientX: 400 + x, clientY: 300 + y };
};

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
const shape = (i: number) => canvas().querySelector(`[data-shape="${i}"]`)!;
async function drag(from: Element, path: Point3[], opts: { altKey?: boolean } = {}) {
  await fireEvent.pointerDown(from, { button: 0, ...at(path[0]!), ...opts });
  for (const p of path.slice(1)) await fireEvent.pointerMove(canvas(), { button: 0, ...at(p), ...opts });
  await fireEvent.pointerUp(canvas(), { button: 0, ...at(path.at(-1)!), ...opts });
}
const tool = async (key: string) => fireEvent.keyDown(window, { key });

describe('Select', () => {
  it('drags a shape on its floor, at the step, as one undo step', async () => {
    const { ask, text } = await editing('switch-poe');
    const tile = first('switch-poe', 'tile');
    await drag(tile, [
      [1, 0.5, 0],
      [1.3, 0.5, 0],
      [1.6, 0.6, 0],
    ]);
    await waitFor(async () => expect(await text()).toContain('tile 0.5 0 0 2 1'));
    expect(ask.mock.calls.map((c) => c[1])).toEqual(expect.arrayContaining(['Move tile in switch-poe']));
    // one undo step for the whole drag
    await fireEvent.click(screen.getByRole('button', { name: 'Undo' }));
    await waitFor(async () => expect(await text()).toBe(source));
  });

  it('lifts with Alt, and nudges and raises by key', async () => {
    const { text } = await editing('switch-poe');
    const tile = first('switch-poe', 'tile');
    await drag(tile, [[1, 0.5, 0], [1, 0.5, 0.3]], { altKey: true });
    await waitFor(async () => expect(await text()).toContain('tile 0 0 0.25 2 1'));
    await fireEvent.keyDown(window, { key: 'ArrowRight' });
    await waitFor(async () => expect(await text()).toContain('tile 0.25 0 0.25 2 1'));
    await fireEvent.keyDown(window, { key: 'ArrowDown', shiftKey: true });
    await waitFor(async () => expect(await text()).toContain('tile 0.25 1 0.25 2 1'));
    await fireEvent.keyDown(window, { key: 'PageDown' });
    await waitFor(async () => expect(await text()).toContain('tile 0.25 1 0 2 1'));
  });

  it('resizes a footprint by a corner handle, with the size beside the pointer', async () => {
    const { text } = await editing('switch-poe');
    const tile = first('switch-poe', 'tile');
    await fireEvent.pointerDown(tile, { button: 0, ...at([1, 0.5, 0]) });
    await fireEvent.pointerUp(canvas(), { button: 0, ...at([1, 0.5, 0]) });
    const corner = await there(() => canvas().querySelector('[data-handle="corner-2"]'));
    await fireEvent.pointerDown(corner, { button: 0, ...at([2, 1, 0]) });
    await fireEvent.pointerMove(canvas(), { button: 0, ...at([3, 2, 0]) });
    expect(await screen.findByText('3 × 2')).toBeTruthy();
    await fireEvent.pointerUp(canvas(), { button: 0, ...at([3, 2, 0]) });
    await waitFor(async () => expect(await text()).toContain('tile 0 0 0 3 2'));
  });

  it('duplicates and deletes by key', async () => {
    const { text } = await editing('switch-poe');
    const tile = first('switch-poe', 'tile');
    await fireEvent.pointerDown(tile, { button: 0, ...at([1, 0.5, 0]) });
    await fireEvent.pointerUp(canvas(), { button: 0, ...at([1, 0.5, 0]) });
    await fireEvent.keyDown(window, { key: 'd', ctrlKey: true });
    await waitFor(async () => expect((await text()).split('tile 0 0 0 2 1').length).toBe(3));
    await fireEvent.keyDown(window, { key: 'Delete' });
    await waitFor(async () => expect((await text()).split('tile 0 0 0 2 1').length).toBe(2));
  });

  it('offers a choice for a value written as an expression, and makes it', async () => {
    const { parts, text } = await editing('switch-l3');
    // the chassis' height is `u * 0.28`, in rack-unit
    await fireEvent.click(within(parts).getByRole('button', { name: /^chassis\s*from rack-unit/ }));
    await within(parts).findByRole('button', { name: /^box/ });
    await fireEvent.click(within(parts).getByRole('button', { name: /^box/ }));
    const handle = await there(() => canvas().querySelector('[data-handle="height"] polygon'));
    const top = at([1, 0.5, 0.28]);
    await fireEvent.pointerDown(handle, { button: 0, ...top });
    await fireEvent.pointerMove(canvas(), { button: 0, clientX: top.clientX, clientY: top.clientY - 12 });
    await fireEvent.pointerUp(canvas(), { button: 0, clientX: top.clientX, clientY: top.clientY - 12 });
    const dialog = await screen.findByRole('dialog', { name: 'This edit needs a choice' });
    expect(within(dialog).getByRole('button', { name: /^Replace with a number/ })).toBeTruthy();
    await fireEvent.click(within(dialog).getByRole('button', { name: /^Edit param u/ }));
    await waitFor(async () => expect(await text()).not.toContain('param u 1\n'));
    expect(await text()).toContain('box 0 0 0 2 1 "u * 0.28"');
  });
});

describe('Box and Tile', () => {
  it('draws a box: a footprint, then its height, into the part drawn in', async () => {
    const { ask, text } = await editing('switch-poe');
    await tool('b');
    await fireEvent.pointerDown(canvas(), { button: 0, ...at([0, 2, 0]) });
    await fireEvent.pointerMove(canvas(), { button: 0, ...at([1, 3, 0]) });
    expect(await screen.findByText('1 × 1')).toBeTruthy();
    await fireEvent.pointerUp(canvas(), { button: 0, ...at([1, 3, 0]) });
    const up = at([1, 3, 0]);
    // 20 units up is half a tile
    await fireEvent.pointerMove(canvas(), { button: 0, clientX: up.clientX, clientY: up.clientY - 20 });
    expect(await screen.findByText('1 × 1 × 0.5')).toBeTruthy();
    await fireEvent.pointerDown(canvas(), { button: 0, clientX: up.clientX, clientY: up.clientY - 20 });
    await waitFor(() => expect(ask).toHaveBeenCalled());
    expect(ask.mock.lastCall![0]).toMatchObject({ kind: 'insert', text: 'box 0 2 0 1 1 0.5', before: null });
    await waitFor(async () => expect(await text()).toContain('    box 0 2 0 1 1 0.5\n  }\n'));
  });

  it('stands a box on the top face it was started on', async () => {
    const { text } = await editing('switch-l3');
    await tool('b');
    // the chassis' top face, the third of its shapes
    await fireEvent.pointerDown(shape(2), { button: 0, ...at([0.25, 0.25, 0.28]) });
    await fireEvent.pointerMove(canvas(), { button: 0, ...at([0.75, 0.75, 0.28]) });
    await fireEvent.pointerUp(canvas(), { button: 0, ...at([0.75, 0.75, 0.28]) });
    const up = at([0.75, 0.75, 0.28]);
    await fireEvent.pointerMove(canvas(), { button: 0, clientX: up.clientX, clientY: up.clientY - 10 });
    await fireEvent.pointerDown(canvas(), { button: 0, clientX: up.clientX, clientY: up.clientY - 10 });
    await waitFor(async () => expect(await text()).toContain('box 0.25 0.25 0.28 0.5 0.5 0.25'));
  });

  it('draws a tile; a click alone draws nothing; Esc stops', async () => {
    const { ask, text } = await editing('switch-poe');
    await tool('t');
    await fireEvent.pointerDown(canvas(), { button: 0, ...at([3, 3, 0]) });
    await fireEvent.pointerUp(canvas(), { button: 0, ...at([3, 3, 0]) });
    expect(ask).not.toHaveBeenCalled();
    await fireEvent.pointerDown(canvas(), { button: 0, ...at([3, 3, 0]) });
    await fireEvent.pointerMove(canvas(), { button: 0, ...at([4, 3.5, 0]) });
    await fireEvent.keyDown(window, { key: 'Escape' });
    await fireEvent.pointerUp(canvas(), { button: 0, ...at([4, 3.5, 0]) });
    expect(ask).not.toHaveBeenCalled();
    await fireEvent.pointerDown(canvas(), { button: 0, ...at([3, 3, 0]) });
    await fireEvent.pointerMove(canvas(), { button: 0, ...at([4, 3.5, 0]) });
    await fireEvent.pointerUp(canvas(), { button: 0, ...at([4, 3.5, 0]) });
    await waitFor(async () => expect(await text()).toContain('tile 3 3 0 1 0.5'));
  });
});
