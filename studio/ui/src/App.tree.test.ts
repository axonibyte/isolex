// The Tree tab (#71) in the window, over the studio's core: the library
// nested as the core's tree has it, opening an icon or a variant, an edit
// marking what it redrew and not drawing again what it did not, a broken
// icon keeping its last drawing, and how a row relates to what is open.
// Timed over the generated library of 500 icons.

import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import App from './App.svelte';
import { core } from './lib/core';
import type { Backend, TreeRow } from './lib/backend';
import sampleStudio from '../../../tests/fixtures/sample.studio.golden.json';
import library500 from '../../../tests/fixtures/library-500.ilx?raw';

afterEach(() => {
  cleanup();
  localStorage.clear();
});

async function opened(backend?: Backend) {
  const b = backend ?? (await core());
  render(App, { backend: b, sample: true });
  await fireEvent.click(screen.getByRole('button', { name: 'Open the sample library' }));
  await screen.findByRole('application', { name: /^Canvas: / });
  await fireEvent.click(screen.getByRole('tab', { name: 'Tree' }));
  const text = async (file = 'sample.ilx') => (await b.source(file))!.text;
  /** Types `to` over the first `from` of the file, as the Source tab would. */
  const typed = async (from: string, to: string, file = 'sample.ilx') => {
    const t = await text(file);
    const at = new TextEncoder().encode(t.slice(0, t.indexOf(from))).length;
    await b.ask({ kind: 'type', file, at, len: new TextEncoder().encode(from).length, text: to }, 'Type');
  };
  return { backend: b, text, typed };
}

const list = () => screen.getByRole('list', { name: 'Icons' });
/** A row by its name: a base's and a variant's say what they are after it. */
const row = (name: string) => within(list()).getByRole('button', { name: new RegExp(`^${name}( |$)`) });
/** The thumbnail drawn on a row. */
const thumb = (name: string) => row(name).querySelector('svg');

/** The rows as listed: each name indented by its depth. */
function listed(ul: Element, depth = 0): string[] {
  const out: string[] = [];
  for (const li of ul.querySelectorAll(':scope > li')) {
    out.push(`${'  '.repeat(depth)}${li.querySelector(':scope > button')!.getAttribute('data-name')}`);
    const sub = li.querySelector(':scope > ul');
    if (sub) out.push(...listed(sub, depth + 1));
  }
  return out;
}
function golden(rows: TreeRow[], depth = 0): string[] {
  return rows.flatMap((r) => [`${'  '.repeat(depth)}${r.name}`, ...golden(r.children, depth + 1)]);
}

describe('the Tree tab', () => {
  it('nests the library as the core’s tree has it', async () => {
    await opened();
    const want = golden((sampleStudio as unknown as { tree: { icons: TreeRow[] } }).tree.icons);
    expect(listed(list())).toEqual(want);
    expect(want).toContain('        switch-poe-down');
    // a variant says which it is; a base has no thumbnail
    expect(row('switch-poe-down').textContent).toContain('variant down');
    expect(thumb('rack-unit')).toBeNull();
    expect(thumb('switch-poe')).not.toBeNull();
  });

  it('opens an icon, or a variant in its axis, and marks it', async () => {
    await opened();
    await fireEvent.click(row('switch-poe-down'));
    await screen.findByRole('application', { name: 'Canvas: switch-poe-down' });
    // the axis is edited: its variants' parts
    const parts = screen.getByRole('complementary', { name: 'Parts' });
    await waitFor(() => expect(within(parts).getAllByText('state-badge').length).toBeGreaterThan(0));
    expect(row('switch-poe-down').getAttribute('aria-current')).toBe('true');
    expect(row('switch-poe').getAttribute('aria-current')).toBeNull();
    await fireEvent.click(row('switch-l3'));
    await screen.findByRole('application', { name: 'Canvas: switch-l3' });
    expect(row('switch-l3').getAttribute('aria-current')).toBe('true');
  });

  it('marks what an edit redrew, and draws nothing again that it did not', async () => {
    const { typed } = await opened();
    const l3 = thumb('switch-l3')!;
    const down = thumb('state-down')!;
    expect(row('switch-poe').querySelector('.redrawn')).toBeNull();
    // switch-poe's tile wider: it and its variants redrawn; the icons
    // after it in the file only moved
    await typed('tile 0 0 0 2 1', 'tile 0 0 0 2.5 1');
    await waitFor(() => expect(row('switch-poe').querySelector('.redrawn')).not.toBeNull());
    expect(row('switch-poe-down').querySelector('.redrawn')).not.toBeNull();
    expect(row('switch-l3').querySelector('.redrawn')).toBeNull();
    expect(row('state-down').querySelector('.redrawn')).toBeNull();
    // the same elements: not drawn again
    expect(thumb('switch-l3')).toBe(l3);
    expect(thumb('state-down')).toBe(down);
  });

  it('keeps a broken icon’s last drawing, its errors on hover', async () => {
    const { typed } = await opened();
    const before = thumb('switch-l3')!.innerHTML;
    await typed('path d="M{x-10}', 'path d="M{x-nope}');
    await waitFor(() => expect(row('switch-l3').querySelector('.broken')).not.toBeNull());
    expect(row('switch-l3').getAttribute('title')).toMatch(/nope/);
    expect(thumb('switch-l3')!.innerHTML).toBe(before);
  });

  it('says how a hovered row relates to what is open', async () => {
    await opened();
    await fireEvent.click(row('switch-poe'));
    await screen.findByRole('application', { name: 'Canvas: switch-poe' });
    const said = () => within(screen.getByRole('tabpanel')).getAllByRole('status').at(-1)!.textContent;
    const hover = (name: string) => fireEvent.pointerEnter(row(name));
    await hover('switch-l2');
    expect(said()).toBe('switch-l2: switch-poe extends it');
    await hover('switch-l3');
    expect(said()).toBe('switch-l3: Shares switch-l2 with switch-poe');
    await hover('state-down');
    expect(said()).toBe('state-down: switch-poe badges it');
    await hover('switch-poe-planned');
    expect(said()).toBe('switch-poe-planned: Variant planned of axis state, of switch-poe, which is being edited');
    await hover('switch-poe');
    expect(said()).toBe('switch-poe: Being edited');
  });
});

describe('the Tree tab at scale', () => {
  it('draws 500 icons, and after an edit draws again only the one it changed', { timeout: 180_000 }, async () => {
    const backend = await core([['library-500.ilx', library500]]);
    const t0 = performance.now();
    const { typed } = await opened(backend);
    await waitFor(() => expect(list().querySelectorAll('svg').length).toBe(602), { timeout: 120_000 });
    const drawn = performance.now() - t0;

    // what changes in the list after one icon's edit
    const touched = new Set<string>();
    const watch = new MutationObserver((records) => {
      for (const r of records) {
        const name = (r.target as Element).closest?.('button')?.getAttribute('data-name') ?? (r.target.parentElement?.closest('button')?.getAttribute('data-name') ?? '');
        if (name) touched.add(name);
      }
    });
    watch.observe(list(), { subtree: true, childList: true, attributes: true, characterData: true });
    const t1 = performance.now();
    await typed('insert "led-row" n=8 z=0.1\n  }\n  part "label" {\n    tile 0.2 0.2 "u * 0.28" 0.6 0.3 fill="accent"\n  }\n}\n\nicon "dev-8"', 'insert "led-row" n=3 z=0.1\n  }\n  part "label" {\n    tile 0.2 0.2 "u * 0.28" 0.6 0.3 fill="accent"\n  }\n}\n\nicon "dev-8"', 'library-500.ilx');
    await waitFor(() => expect(row('dev-7').querySelector('.redrawn')).not.toBeNull(), { timeout: 60_000 });
    const edited = performance.now() - t1;
    watch.disconnect();
    // only dev-7's row
    expect([...touched]).toEqual(['dev-7']);
    console.log(`Tree, 500 icons in jsdom: drawn in ${drawn.toFixed(0)} ms; one icon's edit shown in ${edited.toFixed(0)} ms`);
  });
});
