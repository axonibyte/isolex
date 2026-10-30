// Find (#117): definitions, parts, tokens and text across the library,
// each with where it is written, over the sample library.

import { describe, expect, it } from 'vitest';
import sample from '../../../../tests/fixtures/sample.ilx?raw';
import sampleStudio from '../../../../tests/fixtures/sample.studio.golden.json';
import type { Graph } from './backend';
import { LIMIT, search } from './find';

const graph = (sampleStudio as unknown as { graph: Graph }).graph;
const files = [{ file: 'sample.ilx', text: sample }];
const tokens = ['ink', 'ink-muted', 'outline', 'body-top', 'body-left', 'body-right', 'shadow', 'accent'];
const lineOf = (needle: string) => sample.slice(0, sample.indexOf(needle)).split('\n').length;
const brief = (q: string) => search(q, files, graph, tokens).map((h) => `${h.kind} ${h.label} in ${h.definition} at ${h.line}:${h.col}`);

describe('finding', () => {
  it('definitions and parts by a part of their name, where each is written', () => {
    // in the order written
    expect(brief('state')).toEqual([
      `definition state in state at ${lineOf('axis "state"')}:1`,
      `definition state-down in state-down at ${lineOf('icon "state-down"')}:1`,
      `definition state-planned in state-planned at ${lineOf('icon "state-planned"')}:1`,
      `part part "state-badge" in state at ${lineOf('part "state-badge"')}:5`,
      `part part "state-badge" in state at ${lineOf('variant "planned"') + 2}:5`,
    ]);
    expect(brief('PORTS')).toEqual([`part part "ports" in switch at ${lineOf('part "ports"')}:3`, `part replace "ports" in switch-poe at ${lineOf('replace "ports"')}:3`]);
  });

  it('a token wherever it paints, and not a value that only looks like one', () => {
    expect(brief('acc')).toEqual([`token fill="accent" in state-down at ${lineOf('poly fill="accent"')}:12`]);
    // grant "accent" names it but does not paint with it; corner="ne" is no
    // token, though outline, which "ne" finds, is
    expect(brief('ne').filter((h) => h.startsWith('token'))).toEqual([`token stroke="outline" in switch-poe at ${lineOf('stroke="outline"')}:21`]);
  });

  it('words set in text, and nothing for nothing', () => {
    const text = 'icon "plate" {\n  part "label" {\n    at 0 0 0 {\n      text 0 0 "Rack A1" font="mono"\n    }\n  }\n}\n';
    expect(search('a1', [{ file: 'b.ilx', text }], null, []).map((h) => [h.label, h.line, h.col])).toEqual([['text "Rack A1"', 4, 7]]);
    expect(search('  ', files, graph, tokens)).toEqual([]);
  });

  it('lists at most so many of each kind', () => {
    const text = Array.from({ length: LIMIT + 5 }, (_, i) => `part "p${i}" {}`).join('\n');
    expect(search('p', [{ file: 'c.ilx', text }], null, []).filter((h) => h.kind === 'part')).toHaveLength(LIMIT);
  });
});
