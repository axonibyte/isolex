// Transform (#115) in the window, over the studio's core: each step
// written as the language writes it (SPEC §4), around what is selected
// and about its middle; a step added to a transform selected; what is
// refused; the IR as a fresh compile of the same source gives it, as
// `isolex compile --ir` does; and undo restoring the bytes.

import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import App from './App.svelte';
import { core } from './lib/core';
import type { Backend } from './lib/backend';

const LIB = `lexicon {
  tokens "ink" "outline" "body-top" "body-left" "body-right" "accent"
}

icon "blocks" {
  category "net"
  footprint 6 6 2
  part "body" {
    box 1.5 1.5 0 1 1 1
    box 3 3 0 1 1 1
    tile 0 0 0 1 1
    box 4 1 0 1 1 1
  }
}

icon "flat" {
  category "state"
  canvas 24 24
  part "marks" {
    at 0 0 0 {
      circle 12 12 4 stroke="ink"
      rect 2 2 6 4 stroke="ink"
    }
  }
}
`;

afterEach(() => {
  cleanup();
  localStorage.clear();
});

async function editing(name: string) {
  const backend = await core([['a.ilx', LIB]]);
  render(App, { backend, sample: true });
  await fireEvent.click(screen.getByRole('button', { name: 'Open the sample library' }));
  await screen.findByRole('application', { name: /^Canvas: / });
  await fireEvent.click(screen.getByRole('tab', { name: 'Tree' }));
  await fireEvent.click(within(screen.getByRole('list', { name: 'Icons' })).getByRole('button', { name: new RegExp(`^${name}$`) }));
  await screen.findByRole('application', { name: `Canvas: ${name}` });
  const parts = screen.getByRole('complementary', { name: 'Parts' });
  await waitFor(() => expect(within(parts).queryByText('Reading the definition…')).toBeNull());
  const text = async () => (await backend.source('a.ilx'))!.text;
  /** The rows of a kind, as written: the panel lists the topmost first. */
  const rows = (kind: string) => within(parts).getAllByRole('button', { name: new RegExp(`^${kind}`) }).reverse();
  const choose = async (...list: HTMLElement[]) => {
    for (const [i, r] of list.entries()) await fireEvent.click(r, { shiftKey: i > 0 });
    await fireEvent.keyDown(window, { key: 'A', ctrlKey: true, shiftKey: true });
    return screen.getByRole('tabpanel');
  };
  return { backend, text, rows, choose };
}

const press = (panel: HTMLElement, name: string) => fireEvent.click(within(panel).getByRole('button', { name }));
const undo = () => fireEvent.keyDown(window, { key: 'z', ctrlKey: true });

/** The library as the session compiles it is the library as a fresh
    compile of its text gives it. */
async function compilesAsWritten(backend: Backend, text: string) {
  const fresh = await core([['a.ilx', text]]);
  await fresh.open('fresh');
  expect(await backend.exportIr(null)).toBe(await fresh.exportIr(null));
}

describe('Transform', () => {
  it('rotates a box on the screen, about its middle, and adds a second turn to the same transform', async () => {
    const { backend, text, rows, choose } = await editing('blocks');
    const panel = await choose(rows('box')[0]!);
    await press(panel, 'Rotate clockwise');
    // its corners on the screen run from -34.64 to 34.64 across, 20 to 100 down
    const once = LIB.replace('    box 1.5 1.5 0 1 1 1\n', '    transform rotate=15 origin="0 60" {\n      box 1.5 1.5 0 1 1 1\n    }\n');
    await waitFor(async () => expect(await text()).toBe(once));
    await compilesAsWritten(backend, once);
    // the transform is selected now: another step of its kind adds to it
    await press(panel, 'Rotate clockwise');
    await waitFor(async () => expect(await text()).toBe(once.replace('rotate=15', 'rotate=30')));
    await press(panel, 'Rotate counter-clockwise');
    await press(panel, 'Rotate counter-clockwise');
    await waitFor(async () => expect(await text()).toBe(once.replace('rotate=15', 'rotate=0')));
    for (let i = 0; i < 5; i++) await undo();
    await waitFor(async () => expect(await text()).toBe(LIB));
  });

  it('wraps neighbors in one transform, and others each in their own', async () => {
    const { backend, text, rows, choose } = await editing('blocks');
    const box = rows('box');
    let panel = await choose(box[0]!, box[1]!);
    await fireEvent.input(within(panel).getByRole('spinbutton', { name: 'Width, percent' }), { target: { value: '50' } });
    await press(panel, 'Scale');
    await waitFor(async () => expect(await text()).toMatch(/\n {4}transform scale=0\.5 origin="[-\d. ]+" \{\n {6}box 1\.5 1\.5 0 1 1 1\n {6}box 3 3 0 1 1 1\n {4}\}\n {4}tile/));
    await compilesAsWritten(backend, await text());
    await undo();
    await waitFor(async () => expect(await text()).toBe(LIB));

    // the first and the last box, a tile between: each wrapped, as one undo step
    const again = rows('box');
    panel = await choose(again[0]!, again[2]!);
    await fireEvent.input(within(panel).getByRole('spinbutton', { name: 'Skew x, degrees' }), { target: { value: '10' } });
    await press(panel, 'Skew');
    await waitFor(async () => expect((await text()).match(/transform skew-x=10 origin="[-\d. ]+" \{/g)).toHaveLength(2));
    expect(await text()).toContain('    tile 0 0 0 1 1\n');
    await compilesAsWritten(backend, await text());
    await undo();
    await waitFor(async () => expect(await text()).toBe(LIB));
  });

  it('turns a 3-D shape a quarter in tiles, about the middle of its footprint', async () => {
    const { backend, text, rows, choose } = await editing('blocks');
    const panel = await choose(rows('box')[0]!);
    await press(panel, 'Turn a quarter');
    const turned = LIB.replace('    box 1.5 1.5 0 1 1 1\n', '    turn 90 origin="2 2" {\n      box 1.5 1.5 0 1 1 1\n    }\n');
    await waitFor(async () => expect(await text()).toBe(turned));
    await compilesAsWritten(backend, turned);
    // a quarter back, added to it
    await press(panel, 'Turn a quarter back');
    await waitFor(async () => expect(await text()).toBe(turned.replace('turn 90', 'turn 0')));
    await undo();
    await undo();
    await waitFor(async () => expect(await text()).toBe(LIB));
  });

  it('mirrors flat shapes on the screen, and does not turn them in tiles', async () => {
    const { backend, text, rows, choose } = await editing('flat');
    const panel = await choose(rows('circle')[0]!);
    expect(within(panel).queryByRole('button', { name: 'Turn a quarter' })).toBeNull();
    await press(panel, 'Across');
    const mirrored = LIB.replace('      circle 12 12 4 stroke="ink"\n', '      transform mirror="across" origin="12 12" {\n        circle 12 12 4 stroke="ink"\n      }\n');
    await waitFor(async () => expect(await text()).toBe(mirrored));
    await compilesAsWritten(backend, mirrored);
  });

  it('moves shapes by tiles, as their numbers', async () => {
    const { text, rows, choose } = await editing('blocks');
    const panel = await choose(rows('box')[0]!, rows('tile')[0]!);
    await fireEvent.input(within(panel).getByRole('spinbutton', { name: 'Move by x' }), { target: { value: '0.5' } });
    await fireEvent.input(within(panel).getByRole('spinbutton', { name: 'Move by z' }), { target: { value: '0.25' } });
    await press(panel, 'Move');
    await waitFor(async () => expect(await text()).toBe(LIB.replace('box 1.5 1.5 0 1', 'box 2 1.5 0.25 1').replace('tile 0 0 0 1 1', 'tile 0.5 0 0.25 1 1')));
  });

  it('shows turn handles on a second click, and writes what a drag of one comes to', async () => {
    const { text } = await editing('blocks');
    const canvas = screen.getByRole('application');
    const shape = () => canvas.querySelector('[data-shape="0"]')!;
    const click = async () => {
      await fireEvent.pointerDown(shape(), { button: 0, clientX: 5, clientY: 5 });
      await fireEvent.pointerUp(shape(), { button: 0, clientX: 5, clientY: 5 });
    };
    const handle = (id: string) => canvas.querySelector(`[data-handle="${id}"]`);
    /** A handle dragged from one point of the viewBox to another: the
        canvas, unsized here, shows the viewBox at 1:1 from (400, 300). */
    const drag = async (id: string, from: [number, number], to: [number, number]) => {
      const at = ([x, y]: [number, number]) => ({ clientX: 400 + x, clientY: 300 + y });
      await fireEvent.pointerDown(handle(id)!, { button: 0, ...at(from) });
      await fireEvent.pointerMove(canvas, { button: 0, ...at(to) });
      await fireEvent.pointerUp(canvas, { button: 0, ...at(to) });
    };
    await click();
    expect(handle('rot-0')).toBeNull();
    expect(handle('corner-0')).not.toBeNull();
    // again: the turn handles round it, in place of its own
    await click();
    expect(handle('rot-0')).not.toBeNull();
    expect(handle('scale-x1')).not.toBeNull();
    expect(handle('corner-0')).toBeNull();
    // and again: its own again; and once more, the turn handles
    await click();
    expect(handle('rot-0')).toBeNull();
    await click();

    // its top-left corner, (-34.64, 20) from the middle (0, 60), carried a
    // quarter round clockwise, to (40, 25.36)
    await drag('rot-0', [-34.64, 20], [40, 25.36]);
    const rotated = LIB.replace('    box 1.5 1.5 0 1 1 1\n', '    transform rotate=90 origin="0 60" {\n      box 1.5 1.5 0 1 1 1\n    }\n');
    await waitFor(async () => expect(await text()).toBe(rotated));
    // the transform it is wrapped in keeps them: a side dragged twice as far
    // out scales it across, round the transform
    await waitFor(() => expect(handle('scale-x1')).not.toBeNull());
    const side = handle('scale-x1')!.querySelector('polygon')!.getAttribute('points')!.split(' ').map((p) => p.split(',').map(Number));
    const [sx, sy] = [(side[1]![0]! + side[3]![0]!) / 2, side[0]![1]! + (side[2]![1]! - side[0]![1]!) / 2];
    await drag('scale-x1', [sx, sy], [sx * 2, sy]);
    await waitFor(async () => expect(await text()).toMatch(/\n {4}transform scale="2 1" origin="0 60" \{\n {6}transform rotate=90 origin="0 60" \{\n {8}box 1\.5 1\.5 0 1 1 1\n {6}\}\n {4}\}\n/));
    for (let i = 0; i < 2; i++) await fireEvent.keyDown(window, { key: 'z', ctrlKey: true });
    await waitFor(async () => expect(await text()).toBe(LIB));
  });

  it('says why a part is not transformed', async () => {
    await editing('blocks');
    const parts = screen.getByRole('complementary', { name: 'Parts' });
    await fireEvent.click(within(parts).getByRole('button', { name: /^body/ }));
    await fireEvent.keyDown(window, { key: 'A', ctrlKey: true, shiftKey: true });
    await press(screen.getByRole('tabpanel'), 'Across');
    await waitFor(() => expect(screen.getByText('A part is not transformed: select the shapes in it')).toBeTruthy());
  });
});
