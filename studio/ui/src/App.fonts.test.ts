// Fonts in the editor (#84), in the window over the studio's core, on the
// compiler's text fixture: the Tree's fonts and styles, a font's glyphs
// on the canvas, a glyph edited with the node tool and the pen redrawing
// an icon's text and undone, its advance dragged, its kerning set, and
// the text tool. In jsdom the canvas is never measured: its camera stays
// at scale 1 with the viewBox's origin at (400, 300).

import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import App from './App.svelte';
import { core } from './lib/core';
import type { Backend } from './lib/backend';
import { GLYPH_SIZE } from './lib/font';
import textFixture from '../../../tests/fixtures/text.ilx?raw';

afterEach(() => {
  cleanup();
  localStorage.clear();
});

/** The plain font's em grid on the canvas, as a glyph is shown: an em of
    10 set GLYPH_SIZE tall, its origin a quarter em in from the corner,
    below the ascent of 7. */
const K = GLYPH_SIZE / 10;
const em = (x: number, y: number) => ({ clientX: 400 + (2.5 + x) * K, clientY: 300 + (2.5 + 7 - y) * K });

async function opened() {
  const backend: Backend = await core([['text.ilx', textFixture]]);
  render(App, { backend, sample: true });
  await fireEvent.click(screen.getByRole('button', { name: 'Open the sample library' }));
  await screen.findByRole('application', { name: /^Canvas: / });
  await fireEvent.click(screen.getByRole('tab', { name: 'Tree' }));
  const text = async () => (await backend.source('text.ilx'))!.text;
  return { backend, text };
}

const fonts = () => screen.getByRole('list', { name: 'Fonts' });
const canvas = () => screen.getByRole('application');
const parts = () => screen.getByRole('complementary', { name: 'Parts' });
const there = (find: () => Element | null) =>
  waitFor(() => {
    const el = find();
    if (!el) throw new Error('not there yet');
    return el;
  });

async function openFont(name: string) {
  await fireEvent.click(fonts().querySelector(`[data-name="${name}"]`)!);
  await screen.findByRole('application', { name: `Canvas: ${name}` });
}
async function openGlyph(font: string, glyph: string) {
  await openFont(font);
  await fireEvent.click(await within(parts()).findByRole('button', { name: glyph }));
  await screen.findByRole('application', { name: `Canvas: ${font}, glyph ${glyph}` });
}
async function drag(from: { clientX: number; clientY: number }, to: { clientX: number; clientY: number }, on: Element = canvas()) {
  await fireEvent.pointerDown(on, { button: 0, ...from });
  await fireEvent.pointerMove(canvas(), { button: 0, buttons: 1, ...to });
  await fireEvent.pointerUp(canvas(), { button: 0, ...to });
}

describe('fonts in the Tree', () => {
  it('lists each font with a sample line, its styles under it', async () => {
    await opened();
    const names = within(fonts())
      .getAllByRole('button')
      .map((b) => b.getAttribute('data-name'));
    expect(names).toEqual(['plain', 'plain bold', 'plain italic', 'block', 'block bold']);
    // each with its sample, set in it
    await waitFor(() => expect(within(fonts()).getAllByRole('button')[0]!.querySelector('svg')).not.toBeNull());
    // a wide icon's thumbnail fits its box: port-label is twice as wide
    const thumb = within(screen.getByRole('list', { name: 'Icons' }))
      .getAllByRole('button')
      .find((b) => b.getAttribute('data-name') === 'port-label')!
      .querySelector('svg')!;
    expect(Number(thumb.getAttribute('width'))).toBeLessThanOrEqual(40);
    // outside the app, fonts are imported in it
    expect(screen.getByRole('button', { name: 'Import…' }).hasAttribute('disabled')).toBe(true);
  });
});

describe('a font on the canvas', () => {
  it('sets out its glyphs, ligatures and notdef among them, and its metrics in Design', async () => {
    await opened();
    await openFont('plain');
    expect(within(parts()).getByRole('heading', { name: 'Glyphs' })).toBeTruthy();
    const listed = within(parts()).getAllByRole('button', { name: /^(space|[0-9A-Za-z-]+|notdef)$/ });
    expect(listed.map((b) => b.textContent)).toEqual(expect.arrayContaining(['space', 'A', '--', 'notdef']));
    // drawn: a glyph a cell, as text draws it
    await waitFor(() => expect(canvas().querySelectorAll('[data-shape]').length).toBeGreaterThan(20));
    // and in a style, switched on
    const regular = canvas().innerHTML;
    await fireEvent.click(within(screen.getByRole('group', { name: 'Styles shown' })).getByRole('button', { name: 'bold' }));
    await waitFor(() => expect(canvas().innerHTML).not.toBe(regular));
    expect(within(screen.getByRole('group', { name: 'Styles shown' })).getByRole('button', { name: 'bold' }).getAttribute('aria-pressed')).toBe('true');
    await fireEvent.click(screen.getByRole('tab', { name: 'Design' }));
    expect((screen.getByRole('textbox', { name: 'em' }) as HTMLInputElement).value).toBe('10');
    expect(screen.getByRole('button', { name: 'stroke' }).getAttribute('aria-pressed')).toBe('true');
    expect(screen.getByRole('button', { name: 'Export font…' }).hasAttribute('disabled')).toBe(true);
  });

  it('opens a glyph from its cell', async () => {
    await opened();
    await openFont('plain');
    // the A is the tenth cell, as the font writes it
    const cells = await there(() => canvas().querySelector('[data-shape]'));
    expect(cells).not.toBeNull();
    const a = await within(parts()).findByRole('button', { name: 'A' });
    await fireEvent.click(a);
    await screen.findByRole('application', { name: 'Canvas: plain, glyph A' });
    expect(canvas().querySelectorAll('.guide').length).toBe(5);
  });
});

describe('a glyph edited', () => {
  it('moves a point with the node tool, redraws the icons that set it, and undoes both', async () => {
    const { text } = await opened();
    const before = await text();
    await openGlyph('plain', 'A');
    await fireEvent.keyDown(window, { key: 'a' });
    // its one path, selected in the panel, then its first point
    const a = within(parts()).getByRole('button', { name: 'A' }).closest('li')!;
    await fireEvent.click(within(a as HTMLElement).getByRole('button', { name: /^path/ }));
    const p0 = await there(() => canvas().querySelector('[data-handle="pt-0"]'));
    await drag(em(0, 0), em(0, 1), p0);
    await waitFor(async () => expect(await text()).toContain('glyph "A" advance=5.5 { path d="M0,1 L2,7 L4,0 M0.8,2.5 L3.2,2.5" width="weight" }'));
    // the icons that set an A are drawn again
    await fireEvent.click(screen.getByRole('tab', { name: 'Tree' }));
    const shaping = () => within(screen.getByRole('list', { name: 'Icons' })).getByRole('button', { name: /^shaping/ });
    await waitFor(() => expect(shaping().querySelector('.redrawn')).not.toBeNull());
    await fireEvent.click(screen.getByRole('button', { name: 'Undo' }));
    await waitFor(async () => expect(await text()).toBe(before));
  });

  it('draws a path in the glyph with the pen, in its em grid', async () => {
    const { text } = await opened();
    await openGlyph('plain', 'o');
    await fireEvent.keyDown(window, { key: 'p' });
    await fireEvent.pointerDown(canvas(), { button: 0, ...em(1, 1) });
    await fireEvent.pointerUp(canvas(), { button: 0, ...em(1, 1) });
    await fireEvent.pointerDown(canvas(), { button: 0, ...em(3, 4) });
    await fireEvent.pointerUp(canvas(), { button: 0, ...em(3, 4) });
    await fireEvent.keyDown(window, { key: 'Enter' });
    await waitFor(async () => expect(await text()).toMatch(/glyph "o" advance=5\.5 \{[^}]*path d="M1,1 L3,4"/));
  });

  it('sets its advance by dragging the advance line', async () => {
    const { text } = await opened();
    await openGlyph('plain', 'V');
    const advance = await there(() => canvas().querySelector('[data-handle="advance"]'));
    await drag(em(5.5, 0), em(6.5, 0), advance);
    await waitFor(async () => expect(await text()).toContain('glyph "V" advance=6.5 {'));
  });

  it('lists its kerning, set by number or by dragging the right-hand glyph', async () => {
    const { text } = await opened();
    await openGlyph('plain', 'A');
    await fireEvent.click(screen.getByRole('tab', { name: 'Design' }));
    const pair = await screen.findByRole('group', { name: 'Kern A V' });
    const space = within(pair).getByRole('textbox', { name: 'Space between A and V' }) as HTMLInputElement;
    expect(space.value).toBe('-1');
    await fireEvent.input(space, { target: { value: '-1.5' } });
    await fireEvent.change(space);
    await waitFor(async () => expect(await text()).toContain('kern "A" "V" -1.5'));
    // dragged right by an em grid unit: 6.4 pixels at this size
    const slider = await within(screen.getByRole('group', { name: 'Kern A V' })).findByRole('slider');
    await fireEvent.pointerDown(slider, { button: 0, clientX: 100 });
    await fireEvent.pointerMove(slider, { button: 0, clientX: 106.4 });
    await fireEvent.pointerUp(slider, { button: 0, clientX: 106.4 });
    await waitFor(async () => expect(await text()).toContain('kern "A" "V" -0.5'));
  });
});

describe('the text tool', () => {
  it('sets words in the library’s font, changed in Design', async () => {
    const { text } = await opened();
    await fireEvent.click(within(screen.getByRole('list', { name: 'Icons' })).getByRole('button', { name: /^port-label/ }));
    await screen.findByRole('application', { name: 'Canvas: port-label' });
    await fireEvent.keyDown(window, { key: 'x' });
    // away from the plate's corners, which would pull it at this zoom
    await fireEvent.pointerDown(canvas(), { button: 0, clientX: 424, clientY: 308 });
    await fireEvent.pointerUp(canvas(), { button: 0, clientX: 424, clientY: 308 });
    await waitFor(async () => expect(await text()).toContain('text 24 8 "Text" font="plain" size=8'));
    await fireEvent.click(screen.getByRole('tab', { name: 'Design' }));
    const words = await screen.findByRole('textbox', { name: 'Words' });
    await fireEvent.input(words, { target: { value: 'AV' } });
    await fireEvent.change(words);
    await waitFor(async () => expect(await text()).toContain('text 24 8 AV font="plain" size=8'));
    await fireEvent.click(screen.getByRole('button', { name: 'middle' }));
    await waitFor(async () => expect(await text()).toContain('text 24 8 AV font="plain" size=8 anchor=middle'));
    await fireEvent.click(within(screen.getByRole('group', { name: 'Styles' })).getByRole('button', { name: 'bold' }));
    await waitFor(async () => expect(await text()).toContain('anchor=middle style=bold'));
  });
});
