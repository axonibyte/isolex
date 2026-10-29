// A node's text, as far as the Design tab writes it (#83): its arguments
// found in it, one replaced, and the node wrapped in a `repeat`. Pure;
// every other edit is the core's.

import { clean } from './snap';

/** Where each of a node's arguments is in its text, before its
    properties and its children. */
export function argSpans(text: string): { start: number; end: number }[] {
  const out: { start: number; end: number }[] = [];
  let i = 0;
  const space = () => {
    for (;;) {
      if (i < text.length && /[ \t]/.test(text[i]!)) i++;
      // a line continued
      else if (text[i] === '\\' && /^\\[ \t]*\r?\n/.test(text.slice(i))) i = text.indexOf('\n', i) + 1;
      else break;
    }
  };
  // the name
  while (i < text.length && !/[\s{;]/.test(text[i]!)) i++;
  for (;;) {
    space();
    if (i >= text.length || /[\r\n{;}]/.test(text[i]!)) break;
    const start = i;
    if (text[i] === '"') {
      i++;
      while (i < text.length && text[i] !== '"') i += text[i] === '\\' ? 2 : 1;
      i++;
    } else {
      // a type annotation, then the value
      if (text[i] === '(') i = text.indexOf(')', i) + 1;
      while (i < text.length && !/[\s{;=]/.test(text[i]!)) i++;
      if (text[i] === '=') break;
      if (text[i] === '"') {
        i++;
        while (i < text.length && text[i] !== '"') i += text[i] === '\\' ? 2 : 1;
        i++;
      }
    }
    out.push({ start, end: i });
  }
  return out;
}

/** The node with argument `k` written as `value`. */
export function withArg(text: string, k: number, value: string): string | null {
  const a = argSpans(text)[k];
  return a ? text.slice(0, a.start) + value + text.slice(a.end) : null;
}

/** The argument `k` as written, unquoted. */
export function argText(text: string, k: number): string | null {
  const a = argSpans(text)[k];
  if (!a) return null;
  const t = text.slice(a.start, a.end);
  return t.startsWith('"') ? t.slice(1, -1) : t;
}

/** A name for a repeat's counter not already a word of `text`. */
export function counter(text: string): string {
  for (const v of ['i', 'j', 'k', 'n', 'm']) if (!new RegExp(`\\b${v}\\b`).test(text)) return v;
  return 'copy';
}

/** The node, at `indent`, in a repeat of `count` copies, argument `k`
    (a number now) stepping by `step` from one copy to the next. */
export function inRepeat(text: string, indent: string, k: number, value: number, count: number, step: number, v = counter(text)): string | null {
  const node = withArg(text, k, `"${clean(value)} + ${v} * ${clean(step)}"`);
  if (node === null) return null;
  const nl = text.includes('\r\n') ? '\r\n' : '\n';
  const unit = /^\t/.test(indent) ? '\t' : '  ';
  // its own lines after the first keep their place, one step in
  const body = node
    .split(/\r?\n/)
    .map((l, i) => (i === 0 ? `${indent}${unit}${l}` : `${unit}${l}`))
    .join(nl);
  return `repeat ${count} "${v}" {${nl}${body}${nl}${indent}}`;
}

/** What a repeated argument steps by, and from where, if it is written
    as `"a + v * s"`. */
export function stepOf(text: string, v: string): { from: number; by: number } | null {
  const m = text.match(new RegExp(`^\\s*(-?[\\d.]+)\\s*\\+\\s*${v}\\s*\\*\\s*(-?[\\d.]+)\\s*$`));
  return m ? { from: Number(m[1]), by: Number(m[2]) } : null;
}

/** A node's properties as written before its children, unquoted. */
export function propsOf(text: string): Record<string, string> {
  const head = text.slice(0, braceOf(text));
  const out: Record<string, string> = {};
  for (const m of head.matchAll(/([A-Za-z][\w-]*)=("((?:[^"\\]|\\.)*)"|[^\s{;]+)/g)) out[m[1]!] = m[3] ?? m[2]!;
  return out;
}

/** Where a node's children begin, outside its strings: its length if it
    has none. */
function braceOf(text: string): number {
  let quoted = false;
  for (let i = 0; i < text.length; i++) {
    const c = text[i]!;
    if (c === '\\' && quoted) i++;
    else if (c === '"') quoted = !quoted;
    else if (c === '{' && !quoted) return i;
  }
  return text.length;
}
