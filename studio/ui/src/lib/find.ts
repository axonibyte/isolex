// Find (#117, docs/studio-interface.md, Find), as Inkscape's Find dialog
// (Ctrl+F): definitions, parts, tokens and text across the library, by a
// part of their name or words, each with the place it is written. Pure.

import type { Graph } from './backend';

export interface Hit {
  kind: 'definition' | 'part' | 'token' | 'text';
  /** What it says in the list: `part "ports"`, `fill="accent"`. */
  label: string;
  /** The definition it is written in, if any. */
  definition: string | null;
  file: string;
  /** Where, from 1. */
  line: number;
  col: number;
}

/** How many of each kind are listed. */
export const LIMIT = 50;

/** Each line's start, by character, to say where an offset is. */
function lines(text: string): number[] {
  const out = [0];
  for (let i = 0; i < text.length; i++) if (text[i] === '\n') out.push(i + 1);
  return out;
}

function at(starts: number[], i: number): { line: number; col: number } {
  let lo = 0;
  let hi = starts.length - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (starts[mid]! <= i) lo = mid;
    else hi = mid - 1;
  }
  return { line: lo + 1, col: i - starts[lo]! + 1 };
}

/** Everything in `files` that `query` finds, by kind: definitions, parts,
    tokens and text, each in the order written, at most LIMIT of each. */
export function search(query: string, files: { file: string; text: string }[], graph: Graph | null, tokens: string[]): Hit[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  const has = (s: string) => s.toLowerCase().includes(q);
  const out: Record<Hit['kind'], Hit[]> = { definition: [], part: [], token: [], text: [] };
  const known = new Set(tokens.filter(has));
  for (const { file, text } of files) {
    const starts = lines(text);
    const bytes = new TextEncoder().encode(text);
    // a definition's span is in bytes: its characters, by what precedes it
    const defs = (graph?.definitions ?? [])
      .filter((d) => d.file === file)
      .map((d) => {
        const from = new TextDecoder().decode(bytes.slice(0, d.span.start)).length;
        const to = from + new TextDecoder().decode(bytes.slice(d.span.start, d.span.start + d.span.len)).length;
        return { name: d.name, from, to };
      });
    const inside = (i: number) => defs.find((d) => i >= d.from && i < d.to)?.name ?? null;
    const push = (kind: Hit['kind'], label: string, i: number) => {
      if (out[kind].length < LIMIT) out[kind].push({ kind, label, definition: kind === 'definition' ? label : inside(i), file, ...at(starts, i) });
    };
    for (const d of defs) if (has(d.name)) push('definition', d.name, d.from);
    for (const m of text.matchAll(/\b(part|replace|remove)\s+"([^"]*)"/g)) if (has(m[2]!)) push('part', `${m[1]} "${m[2]}"`, m.index!);
    for (const m of text.matchAll(/([A-Za-z][\w-]*)="([^"]*)"/g)) if (known.has(m[2]!)) push('token', `${m[1]}="${m[2]}"`, m.index!);
    for (const m of text.matchAll(/\btext\s+[^\n"]*"((?:[^"\\]|\\.)*)"/g)) if (has(m[1]!)) push('text', `text "${m[1]}"`, m.index!);
  }
  return [...out.definition, ...out.part, ...out.token, ...out.text];
}
