// Managing a library (#72) in the window, over the studio's core: the
// Library's files and definitions made, renamed and moved; Lexicon
// Properties' tokens, swatches, widths, reserved tokens and limits; a
// change on disk loaded, or flagged with unsaved edits and settled; and
// the welcome panel's recent libraries.

import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import App from './App.svelte';
import { core } from './lib/core';
import type { Backend } from './lib/backend';
import source from '../../../tests/fixtures/sample.ilx?raw';

afterEach(() => {
  cleanup();
  localStorage.clear();
});

async function opened() {
  const backend: Backend = await core();
  render(App, { backend, sample: true });
  await fireEvent.click(screen.getByRole('button', { name: 'Open the sample library' }));
  await screen.findByRole('application', { name: /^Canvas: / });
  const text = async (file = 'sample.ilx') => (await backend.source(file))?.text ?? null;
  return { backend, text };
}

const dialog = (name: string) => screen.getByRole('dialog', { name });
async function type(field: HTMLElement, value: string, key?: string) {
  await fireEvent.input(field, { target: { value } });
  if (key) await fireEvent.keyDown(field, { key });
  else await fireEvent.change(field);
}

describe('the Library', () => {
  it('lists each file with its definitions, and renames one everywhere it is named', async () => {
    const { text } = await opened();
    await fireEvent.click(screen.getByRole('button', { name: 'Library' }));
    const lib = dialog('Library');
    const defs = within(lib).getByRole('list', { name: 'Definitions in sample.ilx' });
    expect(within(defs).getByRole('button', { name: 'switch-l2' })).toBeTruthy();
    await fireEvent.click(within(lib).getByRole('button', { name: 'Rename switch-l2' }));
    await type(within(lib).getByRole('textbox', { name: 'New name for switch-l2' }), 'switch-access', 'Enter');
    await waitFor(async () => expect(await text()).toContain('icon "switch-access" {'));
    expect(await text()).toContain('extends "switch-access"');
    expect(within(lib).getByRole('status').textContent).toBe('Renamed switch-l2 to switch-access, and every reference to it');
    // a name taken is refused, with one that is free
    await fireEvent.click(within(lib).getByRole('button', { name: 'Rename switch-l3' }));
    await type(within(lib).getByRole('textbox', { name: 'New name for switch-l3' }), 'switch-poe', 'Enter');
    await waitFor(() => expect(within(lib).getByRole('status').textContent).toContain('switch-poe-2'));
  });

  it('makes a file and a definition in it, and moves a definition there', async () => {
    const { text } = await opened();
    await fireEvent.click(screen.getByRole('button', { name: 'Library' }));
    const lib = dialog('Library');
    await type(within(lib).getByRole('textbox', { name: 'New file path' }), 'racks', 'Enter');
    await waitFor(() => expect(within(lib).getByRole('list', { name: 'Definitions in racks.ilx' })).toBeTruthy());
    await type(within(lib).getByRole('textbox', { name: 'New definition name' }), 'rack');
    await fireEvent.change(within(lib).getByRole('combobox', { name: 'In file' }), { target: { value: 'racks.ilx' } });
    await fireEvent.click(within(lib).getByRole('button', { name: 'New definition' }));
    await waitFor(async () => expect(await text('racks.ilx')).toContain('icon "rack"'));
    await fireEvent.change(within(lib).getByRole('combobox', { name: 'Move state-planned to' }), { target: { value: 'racks.ilx' } });
    await waitFor(async () => expect(await text('racks.ilx')).toContain('icon "state-planned"'));
    expect(await text()).not.toContain('icon "state-planned"');
  });
});

describe('Lexicon Properties', () => {
  it('adds and removes tokens, and reserves one', async () => {
    const { text } = await opened();
    await fireEvent.click(screen.getByRole('button', { name: 'Lexicon' }));
    const lex = dialog('Lexicon Properties');
    const tokens = await within(lex).findByRole('list', { name: 'Tokens' });
    expect(within(tokens).getAllByRole('listitem')).toHaveLength(8);
    await type(within(lex).getByRole('textbox', { name: 'New token' }), 'virt', 'Enter');
    await waitFor(async () => expect(await text()).toContain('tokens "ink" "ink-muted" "outline" "body-top" "body-left" "body-right" "shadow" "accent" "virt"\n'));
    await fireEvent.click(within(lex).getByRole('button', { name: 'Remove token virt' }));
    await waitFor(async () => expect(await text()).toContain('"shadow" "accent"\n'));
    await fireEvent.click(within(lex).getByRole('tab', { name: 'Reserved' }));
    await fireEvent.click(within(lex).getByRole('checkbox', { name: 'accent' }));
    await waitFor(async () => expect(await text()).toContain('  reserved "accent"\n}'));
  });

  it('colors a token, adding the palette block after the lexicon’s', async () => {
    const { text } = await opened();
    await fireEvent.click(screen.getByRole('button', { name: 'Lexicon' }));
    const lex = dialog('Lexicon Properties');
    await fireEvent.click(within(lex).getByRole('tab', { name: 'Swatches' }));
    const ink = await within(lex).findByLabelText('ink on light');
    await fireEvent.change(ink, { target: { value: '#123456' } });
    await waitFor(async () => expect(await text()).toContain('  stroke-widths 1.2 1.4 1.5 2.2\n}\n\npalette {\n  ink light="#123456" dark="#e4e7eb"\n}\n'));
    // and then where the swatch is
    await waitFor(() => expect(within(lex).getByText('#123456')).toBeTruthy());
    await fireEvent.change(within(lex).getByLabelText('ink on dark'), { target: { value: '#abcdef' } });
    await waitFor(async () => expect(await text()).toContain('ink light="#123456" dark="#abcdef"'));
  });

  it('adds a stroke width in order, and sets and switches off limits', async () => {
    const { text } = await opened();
    await fireEvent.click(screen.getByRole('button', { name: 'Lexicon' }));
    const lex = dialog('Lexicon Properties');
    await fireEvent.click(within(lex).getByRole('tab', { name: 'Stroke widths' }));
    await type(within(lex).getByRole('spinbutton', { name: 'New stroke width' }), '1.3');
    await fireEvent.click(within(lex).getByRole('button', { name: 'Add width' }));
    await waitFor(async () => expect(await text()).toContain('stroke-widths 1.2 1.3 1.4 1.5 2.2'));
    await fireEvent.click(within(lex).getByRole('tab', { name: 'Limits' }));
    await type(within(lex).getByRole('spinbutton', { name: 'shape-limit' }), '500');
    await waitFor(async () => expect(await text()).toContain('  shape-limit 500\n}'));
    const off = within(lex).getAllByRole('checkbox', { name: 'no limit' })[1]!;
    await fireEvent.click(off);
    await waitFor(async () => expect(await text()).toContain('  repeat-limit #null\n}'));
  });
});

describe('a change on disk', () => {
  it('is loaded with nothing unsaved, and marked as an edit is', async () => {
    const { backend, text } = await opened();
    await backend.outside!([['sample.ilx', source.replace('param ports 12', 'param ports 6')]]);
    await waitFor(async () => expect(await text()).toContain('param ports 6'));
    expect(screen.queryByRole('alert')).toBeNull();
    await fireEvent.click(screen.getByRole('tab', { name: 'Tree' }));
    const l3 = within(screen.getByRole('list', { name: 'Icons' })).getByRole('button', { name: 'switch-l3' });
    expect(l3.querySelector('.redrawn')).not.toBeNull();
  });

  it('with unsaved edits, is flagged; kept, reloaded after asking, or saved over', async () => {
    const { backend, text } = await opened();
    await backend.ask({ kind: 'type', file: 'sample.ilx', at: 0, len: 0, text: '// mine\n' }, 'Type');
    const theirs = source.replace('param ports 12', 'param ports 6');
    await backend.outside!([['sample.ilx', theirs]]);
    const strip = await screen.findByRole('alert');
    expect(strip.textContent).toContain('The library changed on disk (sample.ilx), and you have unsaved changes.');
    // the studio keeps working on its own copy
    expect(await text()).toBe(`// mine\n${source}`);
    await fireEvent.click(within(strip).getByRole('button', { name: 'Reload from disk' }));
    await fireEvent.click(within(strip).getByRole('button', { name: 'Keep mine' }));
    expect(await text()).toBe(`// mine\n${source}`);
    await fireEvent.click(within(strip).getByRole('button', { name: 'Reload from disk' }));
    await fireEvent.click(within(strip).getByRole('button', { name: 'Discard and reload' }));
    await waitFor(async () => expect(await text()).toBe(theirs));
    await waitFor(() => expect(screen.queryByRole('alert')).toBeNull());

    // again, and this time saved over
    await backend.ask({ kind: 'type', file: 'sample.ilx', at: 0, len: 0, text: '// mine\n' }, 'Type');
    await backend.outside!([['sample.ilx', source]]);
    const again = await screen.findByRole('alert');
    await fireEvent.click(within(again).getByRole('button', { name: 'Save over it' }));
    await waitFor(() => expect(screen.queryByRole('alert')).toBeNull());
    expect(await text()).toBe(`// mine\n${theirs}`);
  });
});

describe('the welcome panel', () => {
  it('lists the libraries opened lately, and remembers whether it shows', async () => {
    localStorage.setItem('studio.recent', JSON.stringify([{ path: '/home/me/racks', name: 'racks', definitions: 12, opened: Date.now() }]));
    const backend = await core();
    render(App, { backend });
    const list = screen.getByRole('list', { name: 'Recent libraries' });
    expect(within(list).getByRole('button').textContent).toMatch(/racks.*\/home\/me\/racks.*12 definitions/);
    // outside the app, a new library is the app's to make
    expect(screen.getByRole('button', { name: 'New library…' }).hasAttribute('disabled')).toBe(true);
    const startup = screen.getByRole('checkbox', { name: 'Show this at startup' }) as HTMLInputElement;
    expect(startup.checked).toBe(true);
    await fireEvent.click(startup);
    expect(localStorage.getItem('studio.welcome')).toBe('false');
  });
});
