// The Design and Preview tabs, Undo History and Problems (#70) in the
// window, over the studio's core. In jsdom the canvas is never measured,
// so its camera stays at scale 1 with the viewBox's origin at (400, 300).

import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import App from './App.svelte';
import { core } from './lib/core';
import { apply, type Point3 } from './lib/geometry';
import { PROJECTION } from './lib/tools.svelte';
import source from '../../../tests/fixtures/sample.ilx?raw';
import sampleStudio from '../../../tests/fixtures/sample.studio.golden.json';

const sources = sampleStudio.sources as Record<string, { kind: string }[]>;

afterEach(() => {
  cleanup();
  localStorage.clear();
});

const at = (p: Point3) => {
  const [x, y] = apply(PROJECTION, p);
  return { clientX: 400 + x, clientY: 300 + y };
};

async function editing(name: string, tab = 'Design') {
  const backend = await core();
  render(App, { backend, sample: true });
  await fireEvent.click(screen.getByRole('button', { name: 'Open the sample library' }));
  await screen.findByRole('application', { name: /^Canvas: / });
  await fireEvent.click(screen.getByRole('tab', { name: 'Tree' }));
  await fireEvent.click(within(screen.getByRole('list', { name: 'Icons' })).getByRole('button', { name: new RegExp(`^${name}$`) }));
  await screen.findByRole('application', { name: `Canvas: ${name}` });
  const parts = screen.getByRole('complementary', { name: 'Parts' });
  await waitFor(() => expect(within(parts).queryByText('Reading the definition…')).toBeNull());
  await fireEvent.click(screen.getByRole('tab', { name: tab }));
  const text = async () => (await backend.source('sample.ilx'))!.text;
  return { backend, text };
}

const canvas = () => screen.getByRole('application');
const pane = () => screen.getByRole('tabpanel');
const shape = (i: number) => canvas().querySelector(`[data-shape="${i}"]`)!;
const first = (icon: string, kind: string) => shape(sources[icon]!.findIndex((o) => o.kind === kind));
async function click(where: { clientX: number; clientY: number }, on: Element) {
  await fireEvent.pointerDown(on, { button: 0, ...where });
  await fireEvent.pointerUp(canvas(), { button: 0, ...where });
}
/** Types into a field and leaves it, as a change. */
async function type(field: HTMLElement, value: string) {
  await fireEvent.input(field, { target: { value } });
  await fireEvent.change(field);
}
const selectTile = () => click(at([1, 0.5, 0]), first('switch-poe', 'tile'));

describe('Design, with a shape selected', () => {
  it('shows its numbers as written, and sets one, a number or an expression', async () => {
    const { text } = await editing('switch-poe');
    await selectTile();
    const w = await within(pane()).findByRole('textbox', { name: 'w' });
    expect((w as HTMLInputElement).value).toBe('2');
    expect(within(pane()).getAllByRole('textbox').map((t) => t.closest('label')?.querySelector('.label')?.textContent)).toEqual(['x', 'y', 'z', 'w', 'd']);
    await type(w, '3');
    await waitFor(async () => expect(await text()).toContain('tile 0 0 0 3 1 fill="body-top"'));
    await type(within(pane()).getByRole('textbox', { name: 'x' }), 'ports / 24');
    await waitFor(async () => expect(await text()).toContain('tile "ports / 24" 0 0 3 1'));
    // and the field shows it as written
    await waitFor(() => expect((within(pane()).getByRole('textbox', { name: 'x' }) as HTMLInputElement).value).toBe('ports / 24'));
  });

  it('sets its opacity, and changes its order', async () => {
    const { text } = await editing('switch-poe');
    await selectTile();
    await type(await within(pane()).findByRole('spinbutton', { name: 'Opacity' }), '0.5');
    await waitFor(async () => expect(await text()).toContain('dash="3 2.5" opacity=0.5\n'));
    // the tile is drawn first in its part: there is nothing behind it
    expect(within(pane()).getByRole('button', { name: 'Send back' }).hasAttribute('disabled')).toBe(true);
    await fireEvent.click(within(pane()).getByRole('button', { name: 'Bring forward' }));
    await waitFor(async () => expect(await text()).toMatch(/cylinder 0\.5 0\.5 0 0\.4 0\.12[^\n]*\n\s+tile 0 0 0 2 1/));
    // and it stays selected
    expect(within(pane()).getByRole('button', { name: 'Send back' }).hasAttribute('disabled')).toBe(false);
    await fireEvent.click(within(pane()).getByRole('button', { name: 'Duplicate' }));
    await waitFor(async () => expect((await text()).split('tile 0 0 0 2 1').length).toBe(3));
    await fireEvent.click(within(pane()).getByRole('button', { name: 'Delete' }));
    await waitFor(async () => expect((await text()).split('tile 0 0 0 2 1').length).toBe(2));
  });

  it('changes a repeat’s step', async () => {
    const { text } = await editing('switch-poe');
    await selectTile();
    await type(await within(pane()).findByRole('spinbutton', { name: 'Copies' }), '3');
    await fireEvent.click(within(pane()).getByRole('button', { name: 'Repeat' }));
    await waitFor(async () => expect(await text()).toContain('tile "0 + i * 1" 0 0 2 1'));
    // the tile stays selected, its copies and step shown; its step changes
    const step = await within(pane()).findByRole('spinbutton', { name: 'Step' });
    expect((step as HTMLInputElement).value).toBe('1');
    expect((within(pane()).getByRole('spinbutton', { name: 'Copies' }) as HTMLInputElement).value).toBe('3');
    await type(step, '1.5');
    await waitFor(async () => expect(await text()).toContain('tile "0 + i * 1.5" 0 0 2 1'));
  });
});

describe('Design, with nothing selected', () => {
  it('shows a footprint from what is drawn, and edits a canvas', async () => {
    await editing('switch-l3');
    expect(await within(pane()).findByText(/drawn as 2 × 1 × 0\.28/)).toBeTruthy();
    expect(within(pane()).getByText('From rack-unit.')).toBeTruthy();
    cleanup();
    const { text } = await editing('state-planned');
    const width = await within(pane()).findByRole('textbox', { name: 'Width' });
    await type(width, '32');
    await waitFor(async () => expect(await text()).toContain('canvas 32 24\n  describe "Dashed placeholder square."'));
  });

  it('previews a param on the slider, and sets it as the default where asked', async () => {
    const { text } = await editing('switch-l3');
    const ports = await within(pane()).findByRole('group', { name: 'Param ports' });
    expect(within(ports).getByText('from switch')).toBeTruthy();
    const drawn = () => canvas().querySelectorAll('[data-shape]').length;
    const before = drawn();
    await fireEvent.input(within(ports).getByRole('slider', { name: 'Preview ports' }), { target: { value: '4' } });
    // drawn with four ports, nothing written
    await waitFor(() => expect(drawn()).toBe(before - 8));
    expect(await text()).toBe(source);
    expect(screen.getByText(/Previewing ports = 4/)).toBeTruthy();
    await fireEvent.click(within(ports).getByRole('button', { name: 'Reset' }));
    await waitFor(() => expect(drawn()).toBe(before));
    // tried again and set: the core asks where, and it is set there
    await fireEvent.input(within(ports).getByRole('slider', { name: 'Preview ports' }), { target: { value: '6' } });
    await fireEvent.click(await within(ports).findByRole('button', { name: 'Set as default' }));
    const dialog = await screen.findByRole('dialog', { name: 'This edit needs a choice' });
    expect(within(dialog).getByRole('button', { name: /^Edit in switch/ })).toBeTruthy();
    await fireEvent.click(within(dialog).getByRole('button', { name: /^Set it in switch-l3/ }));
    await waitFor(async () => expect(await text()).toContain('  param ports 6\n  part "routing-badge" {'));
    await waitFor(() => expect(drawn()).toBe(before - 6));
    // own now, and not previewed
    await waitFor(() => expect(within(screen.getByRole('group', { name: 'Param ports' })).queryByText('from switch')).toBeNull());
  });
});

describe('Preview', () => {
  it('draws the open icon at five sizes on a light and a dark ground', async () => {
    await editing('switch-poe', 'Preview');
    for (const ground of ['light', 'dark']) {
      const region = screen.getByRole('region', { name: `switch-poe on a ${ground} ground` });
      expect(within(region).getAllByRole('img').map((i) => i.getAttribute('aria-label'))).toEqual([16, 24, 32, 64, 128].map((px) => `switch-poe at ${px} px`));
    }
    // each at its size, by its longer side
    const big = screen.getAllByRole('img', { name: 'switch-poe at 128 px' })[0]!;
    expect(Number(big.getAttribute('width'))).toBeCloseTo(128, 3);
  });
});

describe('Undo History', () => {
  it('lists the steps, and goes back and forward to one', async () => {
    const { text } = await editing('switch-poe');
    await selectTile();
    await type(await within(pane()).findByRole('textbox', { name: 'w' }), '3');
    await waitFor(async () => expect(await text()).toContain('tile 0 0 0 3 1'));
    await type(within(pane()).getByRole('textbox', { name: 'd' }), '2');
    await waitFor(async () => expect(await text()).toContain('tile 0 0 0 3 2'));
    await fireEvent.click(screen.getByRole('button', { name: 'History' }));
    const history = screen.getByRole('dialog', { name: 'Undo history' });
    expect(within(history).getAllByRole('button').map((b) => b.textContent)).toEqual(['Opened', 'Set w of tile in switch-poe', 'Set d of tile in switch-poe']);
    expect(within(history).getByRole('button', { name: 'Set d of tile in switch-poe' }).getAttribute('aria-current')).toBe('step');
    await fireEvent.click(within(history).getByRole('button', { name: 'Opened' }));
    await waitFor(async () => expect(await text()).toBe(source));
    // what was undone is there to go forward to
    await fireEvent.click(screen.getByRole('button', { name: 'History' }));
    const again = screen.getByRole('dialog', { name: 'Undo history' });
    expect(within(again).getByRole('button', { name: 'Opened' }).getAttribute('aria-current')).toBe('step');
    await fireEvent.click(within(again).getByRole('button', { name: 'Set w of tile in switch-poe' }));
    await waitFor(async () => expect(await text()).toContain('tile 0 0 0 3 1 '));
  });
});

describe('Problems', () => {
  it('lists the library’s problems by file, and opens one’s place in the Source tab', async () => {
    await editing('state-planned', 'Tree');
    await fireEvent.click(screen.getByRole('button', { name: /Compiled revision 0 · 1 warning/ }));
    const problems = screen.getByRole('dialog', { name: 'Problems' });
    const file = within(problems).getByRole('region', { name: 'sample.ilx' });
    const items = within(file).getAllByRole('button');
    expect(items).toHaveLength(1);
    expect(items[0]!.textContent).toMatch(/82:3.*grants 'accent'/);
    await fireEvent.click(items[0]!);
    // the Source tab, the caret on that line
    expect(screen.getByRole('tab', { name: 'Source' }).getAttribute('aria-selected')).toBe('true');
    const area = (await screen.findByRole('textbox', { name: /^Source of sample\.ilx/ })) as HTMLTextAreaElement;
    await waitFor(() => expect(area.value.slice(0, area.selectionStart).split('\n').length).toBe(82));
  });
});
