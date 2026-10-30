// Paint (#117): a token picked by the Dropper written into what is drawn
// next, on its first line, before its body.

import { describe, expect, it } from 'vitest';
import { painted, withPaint } from './paint';

describe('a paint written into a node', () => {
  it('goes on its first line, before a body', () => {
    expect(withPaint('rect 0 0 4 4', 'fill', 'accent')).toBe('rect 0 0 4 4 fill="accent"');
    expect(withPaint('poly {\n  pt 0 0\n  pt 4 0\n}', 'stroke', 'ink')).toBe('poly stroke="ink" {\n  pt 0 0\n  pt 4 0\n}');
  });

  it('only on a kind whose paint is its own', () => {
    expect([painted('tile'), painted('path'), painted('box'), painted('cylinder'), painted('shadow')]).toEqual([true, true, false, false, false]);
  });
});
