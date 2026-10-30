// Insert and Badge (#116) in the window, over the studio's core: a
// fragment inserted where the tool clicks or a drag drops it, by its own
// params or moved there on the screen; an icon badged at the nearest
// corner, dragged from the Tree; each selected as one thing, its params,
// corner and scale set from Design and its source opened; and a drop that
// would make a cycle refused, naming the definitions it would join.

import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, createEvent, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import App from './App.svelte';
import { core } from './lib/core';
import { apply, PROJECTION } from './lib/geometry';

const LIB = `lexicon {
  tokens "ink" "outline" "body-top" "body-left" "body-right" "accent"
}

fragment "peg" {
  param cx 0
  param cy 0
  param h 0.5
  part "peg" {
    cylinder "cx" "cy" 0 0.1 "h"
  }
}

fragment "plain" {
  part "a" {
    box 0 0 0 0.5 0.5 0.5
  }
}

fragment "chain" {
  part "a" {
    insert "peg"
  }
}

fragment "dot" {
  param cx 0
  param cy 0
  part "dot" {
    circle "cx" "cy" 2 fill="ink"
  }
}

icon "plate" {
  category "net"
  footprint 4 4 1
  part "body" {
    tile 0 0 0 4 4
  }
}

icon "rack" {
  category "net"
  footprint 2 2 1
  part "body" {
    insert "chain"
  }
}

icon "flag" {
  category "state"
  canvas 24 24
  part "mark" {
    at 0 0 0 {
      circle 12 12 8 stroke="ink"
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
  return { backend, text, parts };
}

const canvas = () => screen.getByRole('application');
/** The canvas, unsized here, shows the viewBox at 1:1 from (400, 300). */
const onScreen = ([x, y]: [number, number]) => ({ clientX: 400 + x, clientY: 300 + y });
const tile = (x: number, y: number) => onScreen(apply(PROJECTION, [x, y, 0]));
async function click(at: { clientX: number; clientY: number }) {
  await fireEvent.pointerDown(canvas(), { button: 0, ...at });
  await fireEvent.pointerUp(canvas(), { button: 0, ...at });
}
/** A drop on the canvas of what a drag from a list or the Tree carries;
    jsdom has no DragEvent, so where it is is given to the event. */
function drop(kind: 'insert' | 'badge', name: string, at: { clientX: number; clientY: number }) {
  const data = JSON.stringify({ kind, name });
  const dataTransfer = { types: ['application/x-isolex-place'], getData: () => data, setData: () => {}, dropEffect: 'copy' };
  const event = createEvent.drop(canvas(), { dataTransfer });
  Object.defineProperties(event, { clientX: { value: at.clientX }, clientY: { value: at.clientY } });
  return fireEvent(canvas(), event);
}
const hint = () => document.querySelector('.hint')!.textContent;
const undo = () => fireEvent.keyDown(window, { key: 'z', ctrlKey: true });

describe('Insert', () => {
  it('writes a fragment where the tool clicks, by its own params, selected as one thing', async () => {
    const { text, parts } = await editing('plate');
    await fireEvent.keyDown(window, { key: 'i' });
    const picker = screen.getByRole('dialog', { name: 'Insert a fragment' });
    expect(within(picker).getAllByRole('button').map((b) => b.textContent!.trim())).toEqual(['chain', 'dot', 'peg', 'plain']);
    await fireEvent.input(within(picker).getByRole('searchbox', { name: 'Find a fragment' }), { target: { value: 'pe' } });
    await fireEvent.click(within(picker).getByRole('button', { name: 'peg' }));
    await click(tile(1, 2));
    const placed = LIB.replace('    tile 0 0 0 4 4\n', '    tile 0 0 0 4 4\n    insert "peg" cx=1 cy=2\n');
    await waitFor(async () => expect(await text()).toBe(placed));
    expect(within(parts).getByRole('button', { name: /^insert/ }).getAttribute('aria-current')).toBe('true');

    // Design: the fragment's params, set as the insert writes them
    await fireEvent.click(screen.getByRole('tab', { name: 'Design' }));
    const h = (await screen.findByRole('textbox', { name: 'h' })) as HTMLInputElement;
    expect([h.value, h.placeholder]).toEqual(['', '0.5']);
    await fireEvent.change(h, { target: { value: '1' } });
    await waitFor(async () => expect(await text()).toBe(placed.replace('cx=1 cy=2', 'cx=1 cy=2 h=1')));
    await undo();
    await undo();
    await waitFor(async () => expect(await text()).toBe(LIB));
  });

  it('moves one with no params for it there on the screen', async () => {
    const { text } = await editing('plate');
    await drop('insert', 'plain', tile(1, 1));
    await waitFor(async () => expect(await text()).toContain('    tile 0 0 0 4 4\n    transform translate="0 40" {\n'));
    expect(await text()).toMatch(/transform translate="0 40" \{\n\s+insert "plain"\n\s+\}/);
    await undo();
    await waitFor(async () => expect(await text()).toBe(LIB));
  });

  it('drops a flat one on the anchor flat shapes are drawn on, at the flat step', async () => {
    const { text } = await editing('flag');
    await drop('insert', 'dot', onScreen([6.2, 17.8]));
    await waitFor(async () => expect(await text()).toContain('      circle 12 12 8 stroke="ink"\n      insert "dot" cx=6 cy=18\n'));
  });

  it('refuses a drop that would make a cycle, naming the definitions it would join', async () => {
    const { text } = await editing('rack');
    // open chain, then peg, from the focus view
    await fireEvent.click(screen.getByRole('radio', { name: 'Focus' }));
    const card = (name: string) => screen.getAllByRole('button').find((b) => b.getAttribute('data-key')?.endsWith(`:${name}`))!;
    await fireEvent.click(card('chain'));
    await waitFor(() => expect(card('peg')).toBeTruthy());
    await fireEvent.click(card('peg'));
    await waitFor(() => expect(screen.getByRole('banner').textContent).toMatch(/fragment\s*peg/));
    await fireEvent.keyDown(window, { key: 'i' });
    const chain = within(screen.getByRole('dialog', { name: 'Insert a fragment' })).getByRole('button', { name: 'chain' }) as HTMLButtonElement;
    expect(chain.disabled).toBe(true);
    expect(chain.title).toBe('That would make a cycle: peg → chain → peg');
    await drop('insert', 'chain', tile(0.5, 0.5));
    await waitFor(() => expect(hint()).toBe('That would make a cycle: peg → chain → peg'));
    expect(await text()).toBe(LIB);
  });
});

describe('Badge', () => {
  it('writes an icon dragged from the Tree at the corner nearest the drop, its corner and scale set from Design', async () => {
    const { backend, text } = await editing('plate');
    const [svg] = await backend.exportSvgs(['plate'], 'light');
    const [x, y, , h] = svg!.svg.match(/viewBox="([^"]+)"/)![1]!.split(' ').map(Number) as [number, number, number, number];
    // dragged from the Tree's row, dropped near the bottom-left corner
    const row = within(screen.getByRole('list', { name: 'Icons' })).getByRole('button', { name: /^flag$/ });
    expect(row.getAttribute('draggable')).toBe('true');
    await drop('badge', 'flag', onScreen([x + 4, y + h - 4]));
    const badged = LIB.replace('    tile 0 0 0 4 4\n', '    tile 0 0 0 4 4\n    badge "flag" corner="sw"\n');
    await waitFor(async () => expect(await text()).toBe(badged));

    await fireEvent.click(screen.getByRole('tab', { name: 'Design' }));
    await fireEvent.change(screen.getByRole('combobox', { name: 'Corner' }), { target: { value: 'ne' } });
    await waitFor(async () => expect(await text()).toBe(badged.replace(' corner="sw"', '')));
    await fireEvent.change(screen.getByRole('textbox', { name: 'Scale' }), { target: { value: '0.4' } });
    await waitFor(async () => expect(await text()).toBe(badged.replace(' corner="sw"', ' scale=0.4')));
    // its source opened from here
    await fireEvent.click(screen.getByRole('button', { name: 'Open flag' }));
    await screen.findByRole('application', { name: 'Canvas: flag' });
  });

  it('places the icon chosen with a click, and refuses what is not an icon', async () => {
    const { text } = await editing('plate');
    await fireEvent.keyDown(window, { key: 'B', shiftKey: true });
    const picker = screen.getByRole('dialog', { name: 'Badge an icon' });
    // plate itself would be a cycle
    expect((within(picker).getByRole('button', { name: 'plate' }) as HTMLButtonElement).disabled).toBe(true);
    await click(tile(2, 2));
    expect(hint()).toBe('Choose an icon to badge: from the list, or drag one in');
    await fireEvent.click(within(picker).getByRole('button', { name: 'flag' }));
    await click(onScreen(apply(PROJECTION, [4, 0, 1])));
    await waitFor(async () => expect(await text()).toContain('    badge "flag"\n'));
    await drop('badge', 'peg', tile(1, 1));
    await waitFor(() => expect(hint()).toBe('peg is not an icon: only an icon is a badge'));
  });
});
