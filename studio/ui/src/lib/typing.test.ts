import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { PAUSE, Typist } from './typing';

describe('Typist', () => {
  let log: string[];
  let t: Typist;
  const type = (s: string) => {
    t.before();
    t.after(s);
  };

  beforeEach(() => {
    vi.useFakeTimers();
    log = [];
    t = new Typist({ begin: () => log.push('begin'), end: () => log.push('end') });
  });
  afterEach(() => vi.useRealTimers());

  it('joins a word into one step', () => {
    type('b');
    type('o');
    type('x');
    expect(log).toEqual(['begin']);
    type(' ');
    expect(log).toEqual(['begin', 'end']);
  });

  it('ends a step at a pause', () => {
    type('b');
    vi.advanceTimersByTime(PAUSE - 1);
    type('o');
    vi.advanceTimersByTime(PAUSE - 1);
    expect(log).toEqual(['begin']);
    vi.advanceTimersByTime(1);
    expect(log).toEqual(['begin', 'end']);
    type('x');
    expect(log).toEqual(['begin', 'end', 'begin']);
  });

  it.each([';', '{', '}', '(', ')', '=', ',', '\n'])('ends a step at %j', (c) => {
    type('a');
    type(c);
    expect(log).toEqual(['begin', 'end']);
  });

  it('ends a step at a deletion', () => {
    type('a');
    type('');
    expect(log).toEqual(['begin', 'end']);
  });

  it('closes once, and only what is open', () => {
    t.close();
    expect(log).toEqual([]);
    type('a');
    t.close();
    t.close();
    vi.runAllTimers();
    expect(log).toEqual(['begin', 'end']);
  });
});
