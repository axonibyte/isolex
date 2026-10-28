// The one splice that turns one text into another: what lies between
// their common start and their common end. Typing sends this, not the
// whole text.

export interface Splice {
  at: number;
  len: number;
  text: string;
}

/** In UTF-8 bytes, as the core counts them. */
const bytes = (s: string): number => new TextEncoder().encode(s).length;

/** The splice of `before` that makes `after`, in byte offsets; none when
    they are the same. */
export function splice(before: string, after: string): Splice | null {
  if (before === after) return null;
  let pre = 0;
  const max = Math.min(before.length, after.length);
  while (pre < max && before[pre] === after[pre]) pre++;
  let suf = 0;
  while (suf < max - pre && before[before.length - 1 - suf] === after[after.length - 1 - suf]) suf++;
  // never inside a surrogate pair
  if (pre > 0 && /[\uD800-\uDBFF]/u.test(before[pre - 1]!)) pre--;
  if (suf > 0 && /[\uDC00-\uDFFF]/u.test(before[before.length - suf]!)) suf--;
  const removed = before.slice(pre, before.length - suf);
  return { at: bytes(before.slice(0, pre)), len: bytes(removed), text: after.slice(pre, after.length - suf) };
}
