// Align and Distribute (#114) in the window, over the studio's core:
// three boxes aligned and spread, writing only their positions, each one
// undo step; to the footprint; a value written as an expression asking;
// and flat shapes in their plane, to the canvas.

import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import App from './App.svelte';
import { core } from './lib/core';

const LIB = `lexicon {
  tokens "ink" "outline" "body-top" "body-left" "body-right" "accent"
}

icon "three" {
  category "net"
  footprint 4 4 2
  param u 1
  part "body" {
    box 0 0 0 1 1 1
    box 2 1 0.5 2 1 1
    box 1 3 0 1 1 2
    box "u * 3" 0 0 1 1 1
  }
}

icon "flat" {
  category "state"
  canvas 24 24
  part "marks" {
    at 0 0 0 {
      circle 12 12 4 stroke="ink"
      rect 2 2 6 4 stroke="ink"
      line 16 18 22 18 stroke="ink"
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
  /** Selects each row in turn, the first with a click and the rest with
      Shift, and opens Align and Distribute. */
  const choose = async (...list: HTMLElement[]) => {
    for (const [i, r] of list.entries()) await fireEvent.click(r, { shiftKey: i > 0 });
    await fireEvent.keyDown(window, { key: 'A', ctrlKey: true, shiftKey: true });
    return screen.getByRole('tabpanel');
  };
  return { text, rows, choose };
}

const press = (panel: HTMLElement, name: string) => fireEvent.click(within(panel).getByRole('button', { name }));
const relativeTo = (panel: HTMLElement, value: string) => fireEvent.change(within(panel).getByRole('combobox', { name: 'Relative to' }), { target: { value } });
const undo = () => fireEvent.keyDown(window, { key: 'z', ctrlKey: true });
/** The library with each `from` written as its `to`. */
const written = (...swaps: [string, string][]) => swaps.reduce((t, [from, to]) => t.replace(from, to), LIB);

describe('Align and Distribute', () => {
  it('aligns three boxes, writing only their positions, as one undo step', async () => {
    const { text, rows, choose } = await editing('three');
    const box = rows('box');
    const panel = await choose(box[0]!, box[1]!, box[2]!);
    expect(within(panel).getByText(/^3 shapes/)).toBeTruthy();
    await press(panel, 'Align x: min');
    await waitFor(async () => expect(await text()).toBe(written(['box 2 1 0.5', 'box 0 1 0.5'], ['box 1 3 0', 'box 0 3 0'])));
    await undo();
    await waitFor(async () => expect(await text()).toBe(LIB));

    // to the first chosen
    await relativeTo(panel, 'first');
    await press(panel, 'Align y: max');
    await waitFor(async () => expect(await text()).toBe(written(['box 2 1 0.5', 'box 2 0 0.5'], ['box 1 3 0', 'box 1 0 0'])));
    await undo();
    await waitFor(async () => expect(await text()).toBe(LIB));

    // to the footprint: their tops at its height
    await relativeTo(panel, 'frame');
    await press(panel, 'Align z: top');
    await waitFor(async () => expect(await text()).toBe(written(['box 0 0 0 1', 'box 0 0 1 1'], ['box 2 1 0.5', 'box 2 1 1'])));
  });

  it('spreads three evenly, by their gaps or their middles', async () => {
    const { text, rows, choose } = await editing('three');
    const box = rows('box');
    const panel = await choose(box[0]!, box[1]!, box[2]!);
    // along y, 0 to 1, 1 to 2 and 3 to 4: gaps of a half
    await press(panel, 'Distribute y: gaps');
    await waitFor(async () => expect(await text()).toBe(written(['box 2 1 0.5', 'box 2 1.5 0.5'])));
    await undo();
    await waitFor(async () => expect(await text()).toBe(LIB));
    // along x, middles 0.5, 3 and 1.5: the last between the other two
    await press(panel, 'Distribute x: middles');
    await waitFor(async () => expect(await text()).toBe(written(['box 1 3 0', 'box 1.25 3 0'])));
  });

  it('asks about a value written as an expression, as any edit does', async () => {
    const { rows, choose } = await editing('three');
    const box = rows('box');
    const panel = await choose(box[0]!, box[3]!);
    expect((within(panel).getByRole('button', { name: 'Distribute x: gaps' }) as HTMLButtonElement).disabled).toBe(true);
    // its x, 3, moved to 0
    await press(panel, 'Align x: min');
    const dialog = await screen.findByRole('dialog', { name: 'This edit needs a choice' });
    expect(within(dialog).getByRole('button', { name: /^Replace with a number/ })).toBeTruthy();
  });

  it('aligns flat shapes along their plane’s x and y, and to the canvas', async () => {
    const { text, rows, choose } = await editing('flat');
    const panel = await choose(rows('circle')[0]!, rows('rect')[0]!, rows('line')[0]!);
    expect(within(panel).getByText(/^3 shapes, flat/)).toBeTruthy();
    expect(within(panel).queryByRole('button', { name: /^Align z/ })).toBeNull();
    await press(panel, 'Align y: top');
    await waitFor(async () => expect(await text()).toBe(written(['circle 12 12 4', 'circle 12 6 4'], ['line 16 18 22 18', 'line 16 2 22 2'])));
    await undo();
    await waitFor(async () => expect(await text()).toBe(LIB));
    await relativeTo(panel, 'frame');
    expect(within(panel).getByRole('option', { name: 'The canvas' })).toBeTruthy();
    await press(panel, 'Align x: right');
    await waitFor(async () => expect(await text()).toBe(written(['circle 12 12 4', 'circle 20 12 4'], ['rect 2 2', 'rect 18 2'], ['line 16 18 22 18', 'line 18 18 24 18'])));
  });

  it('says what to do with nothing selected', async () => {
    await editing('three');
    await fireEvent.click(screen.getByRole('tab', { name: 'Arrange' }));
    expect(screen.getByText(/^Select shapes to line up or spread out/)).toBeTruthy();
    expect((screen.getByRole('button', { name: 'Align x: min' }) as HTMLButtonElement).disabled).toBe(true);
  });
});
