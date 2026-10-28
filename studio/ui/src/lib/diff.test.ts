import { describe, expect, it } from 'vitest';
import { splice, type Splice } from './diff';

const apply = (before: string, s: Splice) => {
  const b = new TextEncoder().encode(before);
  const d = new TextDecoder();
  return d.decode(b.slice(0, s.at)) + s.text + d.decode(b.slice(s.at + s.len));
};

describe('splice', () => {
  it('is none for the same text', () => {
    expect(splice('abc', 'abc')).toBeNull();
  });
  it.each([
    ['box 1 2', 'box 1 23', { at: 7, len: 0, text: '3' }],
    ['box 1 23', 'box 1 2', { at: 7, len: 1, text: '' }],
    ['box 1 2', 'box 4 2', { at: 4, len: 1, text: '4' }],
    ['', 'icon', { at: 0, len: 0, text: 'icon' }],
    ['aaa', 'aaaa', { at: 3, len: 0, text: 'a' }],
  ])('%j to %j', (before, after, want) => {
    expect(splice(before, after)).toEqual(want);
  });
  it('counts in UTF-8 bytes', () => {
    expect(splice('"é" 1', '"é" 2')).toEqual({ at: 5, len: 1, text: '2' });
  });
  it('never splits a surrogate pair', () => {
    // U+1F600 and U+1F601 share their high surrogate
    expect(splice('a\u{1F600}b', 'a\u{1F601}b')).toEqual({ at: 1, len: 4, text: '\u{1F601}' });
  });
  it('makes the text it was asked for, over random edits', () => {
    let seed = 0x9e3779b9;
    const next = () => {
      seed ^= seed << 13;
      seed ^= seed >>> 17;
      seed ^= seed << 5;
      return (seed >>> 0) / 2 ** 32;
    };
    const alphabet = ['a', 'b', ' ', '\n', 'é', '\u{1F600}', '\u{1F601}', '{'];
    const word = () => Array.from({ length: Math.floor(next() * 6) }, () => alphabet[Math.floor(next() * alphabet.length)]).join('');
    for (let i = 0; i < 500; i++) {
      const before = word() + word();
      const after = word() + word();
      const s = splice(before, after);
      expect(s ? apply(before, s) : before, `run ${i}: ${JSON.stringify([before, after])}`).toBe(after);
    }
  });
});
