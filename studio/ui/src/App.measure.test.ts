// Measure, Dropper and Find (#117) in the window, over the studio's core
// and the sample library: a distance shown and nothing written; a paint
// picked and written as only that, on the selection or on what is drawn
// next; and the library searched, each result opening its place.

import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import App from './App.svelte';
import { core } from './lib/core';
import { apply, PROJECTION, type Point3 } from './lib/geometry';
import source from '../../../tests/fixtures/sample.ilx?raw';
import sampleStudio from '../../../tests/fixtures/sample.studio.golden.json';

const sources = sampleStudio.sources as Record<string, { kind: string }[]>;

afterEach(() => {
  cleanup();
  localStorage.clear();
});

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
  return { ask, parts, text };
}

const canvas = () => screen.getByRole('application');
/** The canvas, unsized here, shows the viewBox at 1:1 from (400, 300). */
const at = (p: Point3) => {
  const [x, y] = apply(PROJECTION, p);
  return { clientX: 400 + x, clientY: 300 + y };
};
const shape = (icon: string, kind: string) => canvas().querySelector(`[data-shape="${sources[icon]!.findIndex((o) => o.kind === kind)}"]`)!;
const key = (k: string, more: object = {}) => fireEvent.keyDown(window, { key: k, ...more });
const hint = () => document.querySelector('.hint')!.textContent;

describe('Measure', () => {
  it('shows the distance and angle of a drag, in tiles, and writes nothing', async () => {
    const { ask } = await editing('switch-poe');
    await key('m');
    await fireEvent.pointerDown(canvas(), { button: 0, ...at([3, 3, 0]) });
    await fireEvent.pointerMove(canvas(), { button: 0, ...at([4, 4, 0]) });
    expect(await screen.findByText('1.41 tiles · 45° · Δx 1 Δy 1')).toBeTruthy();
    await fireEvent.pointerUp(canvas(), { button: 0, ...at([4, 4, 0]) });
    // still shown, until Esc
    expect(screen.getByText('1.41 tiles · 45° · Δx 1 Δy 1')).toBeTruthy();
    expect(canvas().querySelectorAll('.preview')).toHaveLength(1);
    await key('Escape');
    expect(screen.queryByText('1.41 tiles · 45° · Δx 1 Δy 1')).toBeNull();
    expect(ask).not.toHaveBeenCalled();
  });
});

describe('Dropper', () => {
  const rect = 'rect -9 -8 18 12 rx=1.5 stroke="ink" width=1.4';

  it('sets the fill picked on what is selected, writing only that', async () => {
    const { parts, text } = await editing('switch-poe');
    await fireEvent.click(within(parts).getByRole('button', { name: /^rect/ }));
    await key('d');
    // the tile is filled body-top
    await fireEvent.pointerDown(shape('switch-poe', 'tile'), { button: 0 });
    await waitFor(async () => expect(await text()).not.toBe(source));
    const now = await text();
    // a token, written bare as KDL writes an identifier
    expect(now).toBe(source.replace(rect, `${rect} fill=body-top`));
    expect(hint()).toBe('Filled one shape with body-top');
  });

  it('sets the stroke with Shift, and leaves what is painted otherwise', async () => {
    const { parts, text } = await editing('switch-poe');
    await fireEvent.click(within(parts).getByRole('button', { name: /^rect/ }));
    await fireEvent.click(within(parts).getByRole('button', { name: /^cylinder/ }), { shiftKey: true });
    await key('d');
    await fireEvent.pointerDown(shape('switch-poe', 'tile'), { button: 0, shiftKey: true });
    await waitFor(async () => expect(await text()).toBe(source.replace(rect, rect.replace('stroke="ink"', 'stroke=outline'))));
    expect(hint()).toBe('Stroked one shape with outline; 1 painted otherwise, left as they are');
  });

  it('keeps the paint picked with nothing selected for what is drawn next', async () => {
    const { text } = await editing('switch-poe');
    await key('d');
    await fireEvent.pointerDown(shape('switch-poe', 'tile'), { button: 0 });
    expect(hint()).toBe('Fill body-top: for what is drawn next');
    expect(await text()).toBe(source);
    await key('t');
    await fireEvent.pointerDown(canvas(), { button: 0, ...at([3, 3, 0]) });
    await fireEvent.pointerMove(canvas(), { button: 0, ...at([4, 3.5, 0]) });
    await fireEvent.pointerUp(canvas(), { button: 0, ...at([4, 3.5, 0]) });
    await waitFor(async () => expect(await text()).toContain('    tile 3 3 0 1 0.5 fill="body-top"\n'));
  });
});

describe('Find', () => {
  it('finds across the library, and opens a definition, or a place in the source', async () => {
    await editing('switch-l3');
    await key('f', { ctrlKey: true });
    let dialog = screen.getByRole('dialog', { name: 'Find' });
    const box = within(dialog).getByRole('searchbox', { name: 'Find in the library' });
    await fireEvent.input(box, { target: { value: 'ports' } });
    const parts = await within(dialog).findByRole('list', { name: 'Parts (2)' });
    expect(within(parts).getAllByRole('button').map((b) => b.querySelector('code')!.textContent)).toEqual(['part "ports"', 'replace "ports"']);
    // its place: the definition opened, the Source tab at the line
    await fireEvent.click(within(parts).getAllByRole('button')[1]!);
    expect(screen.queryByRole('dialog', { name: 'Find' })).toBeNull();
    await screen.findByRole('application', { name: 'Canvas: switch-poe' });
    const editor = (await screen.findByRole('textbox', { name: /^Source of sample\.ilx/ })) as HTMLTextAreaElement;
    await waitFor(() => expect(editor.selectionStart).toBe(source.indexOf('replace "ports"')));

    // a definition opens; Enter opens the first found
    await key('f', { ctrlKey: true });
    dialog = screen.getByRole('dialog', { name: 'Find' });
    await fireEvent.input(within(dialog).getByRole('searchbox'), { target: { value: 'state-d' } });
    await within(dialog).findByRole('list', { name: 'Definitions (1)' });
    await fireEvent.keyDown(within(dialog).getByRole('searchbox'), { key: 'Enter' });
    await screen.findByRole('application', { name: 'Canvas: state-down' });
  });

  it('says when nothing matches', async () => {
    await editing('switch-l3');
    await key('f', { ctrlKey: true });
    await fireEvent.input(screen.getByRole('searchbox', { name: 'Find in the library' }), { target: { value: 'zebra' } });
    expect(screen.getByText('Nothing in the library matches “zebra”.')).toBeTruthy();
    await key('Escape');
    expect(screen.queryByRole('dialog', { name: 'Find' })).toBeNull();
  });
});
