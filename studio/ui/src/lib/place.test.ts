// Insert and Badge (#116): the node a drop writes, the corner a badge
// takes, and what is refused, over the graph fixture with every kind of
// reference.

import { describe, expect, it } from 'vitest';
import graphGolden from '../../../../tests/fixtures/graph.golden.json';
import type { Graph, Row } from './backend';
import { badgeOf, badgeText, cycle, drawsFlat, insertText, nearestCorner, refusal } from './place';

const graph = graphGolden as unknown as Graph;
const row = (kind: string, children: Row[] = []): Row => ({ kind, note: '', node: { file: 'a.ilx', start: 0, len: 1 }, children });

describe('an insert', () => {
  it('sets where it goes as the fragment’s own params, when it has them', () => {
    expect(insertText('led-row', ['n', 'x0', 'y0', 'z'], [1.25, 0.5], [0, 0])).toBe('insert "led-row" x0=1.25 y0=0.5');
    expect(insertText('disk-stack', ['cx', 'cy', 'z', 'n'], [0.5, 1], [0, 0])).toBe('insert "disk-stack" cx=0.5 cy=1');
  });

  it('else is moved there on the screen, or left where it draws at the origin', () => {
    expect(insertText('led', [], [1, 0], [34.641, 20])).toBe('transform translate="34.64 20" {\n  insert "led"\n}');
    expect(insertText('led', ['n'], [0, 0], [0, 0])).toBe('insert "led"');
  });

  it('goes on an anchor when what the fragment holds is flat', () => {
    expect(drawsFlat([row('repeat', [row('circle')])])).toBe(true);
    expect(drawsFlat([row('repeat', [row('facerect')]), row('box')])).toBe(false);
  });
});

describe('a badge', () => {
  it('takes the corner of the frame nearest the drop', () => {
    const vb: [number, number, number, number] = [-70, -20, 140, 100];
    expect(nearestCorner(vb, [50, -10])).toBe('ne');
    expect(nearestCorner(vb, [-50, 70])).toBe('sw');
    expect(nearestCorner(vb, [10, 60])).toBe('se');
    expect(badgeText('down', 'ne')).toBe('badge "down"');
    expect(badgeText('down', 'sw')).toBe('badge "down" corner="sw"');
  });

  it('is of an icon with no badge of its own, through what it extends or opts into', () => {
    // server opts into the axis state, whose variant badges down
    expect(badgeOf(graph, 'server')).toBe('down');
    expect(badgeOf(graph, 'down')).toBeNull();
    expect(badgeOf(graph, 'switch')).toBeNull();
  });
});

describe('what is refused', () => {
  it('a cycle, naming the definitions it would join', () => {
    // led-row inserts led: led inserting led-row would close it
    expect(cycle(graph, 'led', 'led-row')).toEqual(['led', 'led-row', 'led']);
    expect(cycle(graph, 'led-row', 'led-row')).toEqual(['led-row', 'led-row']);
    expect(cycle(graph, 'server', 'led-row')).toBeNull();
    // down is badged through state, which server opts into
    expect(cycle(graph, 'down', 'server')).toEqual(['down', 'server', 'state', 'down']);
  });

  it('says why', () => {
    expect(refusal(graph, 'led', 'led-row', 'insert')).toBe('That would make a cycle: led → led-row → led');
    expect(refusal(graph, 'switch', 'server', 'insert')).toBe('server is not a fragment: only a fragment is inserted');
    expect(refusal(graph, 'switch', 'led', 'badge')).toBe('led is not an icon: only an icon is a badge');
    expect(refusal(graph, 'switch', 'server', 'badge')).toBe('server has a badge of its own (down): a badge’s icon cannot');
    expect(refusal(graph, 'switch', 'down', 'badge')).toBeNull();
    expect(refusal(graph, 'switch', 'led-row', 'insert')).toBeNull();
  });
});
