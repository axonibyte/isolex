// Advanced snapping (#118) in the window, over the studio's core and the
// sample library: the Advanced list beside the step switches each kind of
// target, remembered; a target switched off no longer pulls; and an
// alignment guide is drawn while a point lines up with another shape's.

import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import App from './App.svelte';
import { core } from './lib/core';
import { apply, PROJECTION, type Point3 } from './lib/geometry';

afterEach(() => {
  cleanup();
  localStorage.clear();
});

async function editing(name: string) {
  const backend = await core();
  render(App, { backend, sample: true });
  await fireEvent.click(screen.getByRole('button', { name: 'Open the sample library' }));
  await screen.findByRole('application', { name: /^Canvas: / });
  await fireEvent.click(screen.getByRole('tab', { name: 'Tree' }));
  await fireEvent.click(within(screen.getByRole('list', { name: 'Icons' })).getByRole('button', { name: new RegExp(`^${name}$`) }));
  await screen.findByRole('application', { name: `Canvas: ${name}` });
  const parts = screen.getByRole('complementary', { name: 'Parts' });
  await waitFor(() => expect(within(parts).queryByText('Reading the definition…')).toBeNull());
}

const canvas = () => screen.getByRole('application');
/** The canvas, unsized here, shows the viewBox at 1:1 from (400, 300). */
const at = (p: Point3) => {
  const [x, y] = apply(PROJECTION, p);
  return { clientX: 400 + x, clientY: 300 + y };
};
const key = (k: string) => fireEvent.keyDown(window, { key: k });
const snapped = () => document.querySelector('.tag.snap')?.textContent ?? null;
const advanced = async () => {
  await fireEvent.click(screen.getByRole('button', { name: 'Advanced' }));
  return screen.getByRole('group', { name: 'Snap to' });
};

describe('the Advanced list', () => {
  it('switches each kind of target, remembered for next time', async () => {
    await editing('switch-poe');
    const button = screen.getByRole('button', { name: 'Advanced' });
    expect(button.getAttribute('aria-expanded')).toBe('false');
    const list = await advanced();
    expect(button.getAttribute('aria-expanded')).toBe('true');
    const boxes = within(list).getAllByRole('checkbox') as HTMLInputElement[];
    expect(boxes.map((b) => b.closest('label')!.textContent!.trim())).toEqual([
      'Grid lines and intersections',
      'Corners',
      'Edges',
      'Edge midpoints',
      'Face centers',
      'Anchors',
      'Path nodes',
      'The frame',
      'Alignment guides',
    ]);
    expect(boxes.every((b) => b.checked)).toBe(true);
    await fireEvent.click(within(list).getByRole('checkbox', { name: 'Corners' }));
    expect(JSON.parse(localStorage.getItem('snapTargets')!).corners).toBe(false);

    cleanup();
    await editing('switch-poe');
    const again = await advanced();
    expect((within(again).getByRole('checkbox', { name: 'Corners' }) as HTMLInputElement).checked).toBe(false);
    expect((within(again).getByRole('checkbox', { name: 'Edges' }) as HTMLInputElement).checked).toBe(true);
  });

  it('stops a target switched off from pulling', async () => {
    await editing('switch-poe');
    await key('m');
    // near the tile's corner at (2, 0)
    await fireEvent.pointerDown(canvas(), { button: 0, ...at([3, 3, 0]) });
    await fireEvent.pointerMove(canvas(), { button: 0, ...at([2.05, 0.05, 0]) });
    expect(snapped()).toBe('corner of tile');
    await fireEvent.pointerUp(canvas(), { button: 0, ...at([2.05, 0.05, 0]) });

    const list = await advanced();
    await fireEvent.click(within(list).getByRole('checkbox', { name: 'Corners' }));
    await key('m');
    await fireEvent.pointerDown(canvas(), { button: 0, ...at([3, 3, 0]) });
    await fireEvent.pointerMove(canvas(), { button: 0, ...at([2.05, 0.05, 0]) });
    // the footprint's corner is there too, the frame's
    expect(snapped()).toBe('corner of the footprint');
    await fireEvent.pointerUp(canvas(), { button: 0, ...at([2.05, 0.05, 0]) });

    await fireEvent.click(within(list).getByRole('checkbox', { name: 'The frame' }));
    await key('m');
    await fireEvent.pointerDown(canvas(), { button: 0, ...at([3, 3, 0]) });
    await fireEvent.pointerMove(canvas(), { button: 0, ...at([2.05, 0.05, 0]) });
    // the edge it is on, now
    expect(snapped()).toBe('edge of tile');
  });
});

describe('alignment guides', () => {
  it('draws the guide a point lines up on, while dragging', async () => {
    await editing('switch-poe');
    const list = await advanced();
    // off the grid, so only the guide pulls
    await fireEvent.click(within(list).getByRole('checkbox', { name: 'Grid lines and intersections' }));
    await key('m');
    await fireEvent.pointerDown(canvas(), { button: 0, ...at([3, 3, 0]) });
    await fireEvent.pointerMove(canvas(), { button: 0, ...at([2.05, 3, 0]) });
    expect(snapped()).toBe('aligned with corner of tile');
    const guides = canvas().querySelectorAll('line.aligned');
    expect(guides).toHaveLength(1);
    // from the tile's corner to the point lined up, at x 2
    const [x1, y1, x2, y2] = ['x1', 'y1', 'x2', 'y2'].map((a) => Number(guides[0]!.getAttribute(a)));
    const [ax, ay] = apply(PROJECTION, [2, 0, 0]);
    const [bx, by] = apply(PROJECTION, [2, 3, 0]);
    expect([x1, y1, x2, y2].map((v) => Math.round(v * 1000) / 1000)).toEqual([ax, ay, bx, by].map((v) => Math.round(v * 1000) / 1000));
    expect(await screen.findByText('1 tile · 180° · Δx -1 Δy 0')).toBeTruthy();

    // switched off, nothing lines up
    await fireEvent.pointerUp(canvas(), { button: 0, ...at([2.05, 3, 0]) });
    await fireEvent.click(within(list).getByRole('checkbox', { name: 'Alignment guides' }));
    await key('m');
    await fireEvent.pointerDown(canvas(), { button: 0, ...at([3, 3, 0]) });
    await fireEvent.pointerMove(canvas(), { button: 0, ...at([2.05, 3, 0]) });
    expect(canvas().querySelectorAll('line.aligned')).toHaveLength(0);
    expect(await screen.findByText('0.95 tiles · 180° · Δx -0.95 Δy 0')).toBeTruthy();
  });
});
