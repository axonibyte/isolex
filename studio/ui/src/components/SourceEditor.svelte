<!--
  Adapted from bam (Cambridge-Hackspace/boring-asset-manager) at b2ca54b:
  web/src/components/IconSourceEditor.svelte. Its workings are bam's; it is
  restyled with the studio's tokens in place of Tailwind, sized by its
  container, and calls `onedit` with each change a person makes.

  An editor for isolex source, as a code editor works: a plain textarea, so
  typing, selection and screen readers are the browser's own, over a
  colored copy of the text; line numbers with a mark on each line that has
  a problem, the problem's token underlined; completion of node names,
  properties and the lexicon's names; Tab indents (Esc, then Tab, leaves),
  Enter keeps the indent, Ctrl+/ comments lines.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import { tokenize, offsetOf, rangeAt, type Problem, type TokenKind } from '../lib/isolex/syntax';
  import { complete, type Completion, type Vocabulary } from '../lib/isolex/complete';
  import { indent, outdent, newline, closeBrace, toggleComment, type Change } from '../lib/isolex/edit';

  interface Props {
    value: string;
    /** What the compiler said of this source. */
    problems?: Problem[];
    vocabulary?: Vocabulary;
    /** The editor's accessible name. */
    label: string;
    id?: string;
    readonly?: boolean;
    /** The line to show and mark, such as the selection's. */
    highlight?: number | null;
    /** Each change a person makes, with the text after it. */
    onedit?: (value: string) => void;
    /** Where the caret was put, by a click or a key that moves it, as a
        character offset. */
    oncaret?: (offset: number) => void;
  }

  let {
    value = $bindable(''),
    problems = [],
    vocabulary = { tokens: [], definitions: [], icons: [] },
    label,
    id = 'isolex-' + Math.random().toString(36).slice(2),
    readonly = false,
    highlight = null,
    onedit,
    oncaret,
  }: Props = $props();

  const MOVES = new Set(['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'Home', 'End', 'PageUp', 'PageDown']);
  const caret = () => {
    if (area.selectionStart === area.selectionEnd) oncaret?.(area.selectionStart);
  };

  let area: HTMLTextAreaElement;
  let overlay: HTMLPreElement;
  let gutter: HTMLDivElement;
  let measure: HTMLSpanElement;

  // Esc, then Tab, moves on rather than indenting, so the keyboard is
  // never kept in here.
  let leaving = false;
  let completion = $state<Completion | null>(null);
  let active = $state(0);
  let popup = $state({ left: 0, top: 0 });

  const LINE = 20; // px
  const PAD = 8; // px

  const lines = $derived(value.split('\n').length);

  /** Each problem's range in the text. */
  const ranges = $derived(
    problems.map((p) => {
      const [from, to] = rangeAt(value, offsetOf(value, p.line, p.col));
      return { from, to, severity: p.severity };
    }),
  );

  const marks = $derived.by(() => {
    const byLine = new Map<number, 'error' | 'warning'>();
    for (const p of problems) {
      if (byLine.get(p.line) !== 'error') byLine.set(p.line, p.severity);
    }
    return byLine;
  });

  /** The text as colored pieces, split where a problem's range starts or ends. */
  const segments = $derived.by(() => {
    const cuts = new Set<number>();
    for (const r of ranges) cuts.add(r.from).add(r.to);
    const out: { text: string; kind: TokenKind; squiggle: '' | 'error' | 'warning' }[] = [];
    for (const t of tokenize(value)) {
      const points = [t.start, ...[...cuts].filter((c) => c > t.start && c < t.end).sort((a, b) => a - b), t.end];
      for (let k = 0; k + 1 < points.length; k++) {
        const from = points[k]!;
        const to = points[k + 1]!;
        const hit = ranges.find((r) => r.from <= from && to <= r.to);
        out.push({ text: value.slice(from, to), kind: t.kind, squiggle: hit ? hit.severity : '' });
      }
    }
    return out;
  });

  function follow() {
    overlay.scrollTop = area.scrollTop;
    overlay.scrollLeft = area.scrollLeft;
    gutter.scrollTop = area.scrollTop;
  }

  /** Applies a change as one step of the browser's own undo. */
  function apply(c: Change) {
    area.focus();
    area.setSelectionRange(c.from, c.to);
    let done = false;
    if (typeof document.execCommand === 'function') {
      done = c.insert === '' ? c.from === c.to || document.execCommand('delete') : document.execCommand('insertText', false, c.insert);
    }
    if (!done) {
      area.setRangeText(c.insert, c.from, c.to, 'end');
      area.dispatchEvent(new Event('input', { bubbles: true }));
    }
    value = area.value;
    area.setSelectionRange(c.selStart, c.selEnd);
    onedit?.(value);
  }

  function suggest() {
    const caret = area.selectionStart;
    completion = area.selectionStart === area.selectionEnd ? complete(value, caret, vocabulary) : null;
    active = 0;
    if (!completion) return;
    // under the start of what is being completed, as a code editor puts it
    const before = value.slice(0, completion.from);
    const line = before.split('\n').length - 1;
    const col = before.length - (before.lastIndexOf('\n') + 1);
    const ch = measure?.getBoundingClientRect().width / 10 || 8;
    popup = { left: PAD + col * ch - area.scrollLeft, top: PAD + (line + 1) * LINE - area.scrollTop };
  }

  function accept() {
    const c = completion;
    if (!c) return;
    const s = c.items[active]!;
    const at = c.from;
    completion = null;
    apply({
      from: c.from,
      to: c.to,
      insert: s.insert,
      selStart: s.select ? at + s.select[0] : at + s.insert.length,
      selEnd: s.select ? at + s.select[1] : at + s.insert.length,
    });
  }

  function onkeydown(e: KeyboardEvent) {
    if (readonly) return;
    const s = area.selectionStart;
    const end = area.selectionEnd;
    if (completion) {
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault();
        const n = completion.items.length;
        active = (active + (e.key === 'ArrowDown' ? 1 : n - 1)) % n;
        return;
      }
      if (e.key === 'Enter' || e.key === 'Tab') {
        e.preventDefault();
        accept();
        return;
      }
      if (e.key === 'Escape') {
        e.preventDefault();
        completion = null;
        return;
      }
    }
    if (e.key === ' ' && e.ctrlKey) {
      e.preventDefault();
      suggest();
      return;
    }
    if (e.key === 'Escape') {
      leaving = true;
      return;
    }
    if (e.key === 'Tab') {
      if (leaving || e.ctrlKey || e.altKey || e.metaKey) {
        leaving = false;
        return; // the browser moves focus on
      }
      e.preventDefault();
      apply(e.shiftKey ? outdent(value, s, end) : indent(value, s, end));
      return;
    }
    leaving = false;
    if (e.key === 'Enter' && !e.shiftKey && !e.ctrlKey && !e.altKey && !e.metaKey && !e.isComposing) {
      e.preventDefault();
      apply(newline(value, s, end));
      return;
    }
    if (e.key === '}') {
      const c = closeBrace(value, s, end);
      if (c) {
        e.preventDefault();
        apply(c);
      }
      return;
    }
    if (e.key === '/' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      apply(toggleComment(value, s, end));
    }
  }

  function oninput(e: Event) {
    const typed = (e as InputEvent).data;
    if ((e as InputEvent).inputType === 'insertText' && typed && /[A-Za-z0-9_."=-]$/u.test(typed)) {
      suggest();
    } else {
      completion = null;
    }
    onedit?.(area.value);
  }

  /** Puts the caret on a problem, and it in view. */
  export async function reveal(p: Problem) {
    const [from, to] = rangeAt(value, offsetOf(value, p.line, p.col));
    area.focus();
    area.setSelectionRange(from, to);
    area.scrollTop = Math.max(0, (p.line - 1) * LINE - area.clientHeight / 2);
    await tick();
    follow();
  }

  // the highlighted line, in view
  $effect(() => {
    if (highlight && area) {
      const top = (highlight - 1) * LINE;
      if (top < area.scrollTop || top > area.scrollTop + area.clientHeight - LINE * 2) {
        area.scrollTop = Math.max(0, top - area.clientHeight / 3);
        follow();
      }
    }
  });

  const listId = $derived(id + '-suggestions');
  const helpId = $derived(id + '-help');
  const problemsId = $derived(id + '-problems');
  const errors = $derived(problems.filter((p) => p.severity === 'error').length);
</script>

<div class="editor">
  <div class="frame">
    <div bind:this={gutter} aria-hidden="true" class="gutter">
      {#each { length: lines } as _, i (i)}
        {@const mark = marks.get(i + 1)}
        <div class="ln" class:hl={highlight === i + 1}>
          {#if mark}<span data-mark={mark} class="mark {mark}">{mark === 'error' ? '●' : '▲'}</span>{/if}
          <span class="num">{i + 1}</span>
        </div>
      {/each}
    </div>
    <div class="text">
      {#if highlight}<div class="band" aria-hidden="true" style="top: {PAD + (highlight - 1) * LINE}px"></div>{/if}
      <pre bind:this={overlay} aria-hidden="true" data-overlay class="overlay">{#each segments as seg, k (k)}<span
            class="t-{seg.kind}"
            class:sq-error={seg.squiggle === 'error'}
            class:sq-warning={seg.squiggle === 'warning'}>{seg.text}</span>{/each}{'\n'}</pre>
      <textarea
        bind:this={area}
        bind:value
        {id}
        {readonly}
        aria-label={label}
        aria-describedby="{helpId}{problems.length ? ' ' + problemsId : ''}"
        aria-invalid={errors > 0}
        aria-autocomplete="list"
        aria-controls={completion ? listId : undefined}
        aria-activedescendant={completion ? `${listId}-${active}` : undefined}
        spellcheck="false"
        autocapitalize="off"
        autocomplete="off"
        wrap="off"
        onscroll={follow}
        {onkeydown}
        {oninput}
        onblur={() => (completion = null)}
        onmouseup={caret}
        onkeyup={(e) => {
          if (MOVES.has(e.key) && !completion) caret();
        }}
      ></textarea>
      <span bind:this={measure} aria-hidden="true" class="measure">0000000000</span>
      {#if completion}
        <ul id={listId} role="listbox" aria-label="Suggestions" class="suggest" style="left: {popup.left}px; top: {popup.top}px">
          {#each completion.items as s, k (s.label)}
            <li
              id="{listId}-{k}"
              role="option"
              aria-selected={k === active}
              onmousedown={(e) => {
                e.preventDefault();
                active = k;
                accept();
              }}
            >
              <span class="label">{s.label}</span>
              <span class="detail">{s.detail}</span>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  </div>
  <p id={helpId} class="help">Tab indents; Esc, then Tab, leaves the editor. Ctrl+Space suggests what fits; Ctrl+/ comments lines out.</p>
  <div role="status" class="sr-only">{completion ? `${completion.items.length} suggestions` : ''}</div>
  {#if problems.length}
    <div id={problemsId} class="problems">
      <h3>Problems</h3>
      <ul>
        {#each problems as p, k (k)}
          <li>
            <button type="button" onclick={() => reveal(p)}>
              <span class="mark {p.severity}" aria-hidden="true">{p.severity === 'error' ? '●' : '▲'}</span>
              <span class="at" aria-hidden="true">{p.line}:{p.col}</span>
              <span class="sr-only">{p.severity === 'error' ? 'Error' : 'Warning'}, line {p.line}, column {p.col}:</span>
              <span>{p.message}</span>
            </button>
          </li>
        {/each}
      </ul>
    </div>
  {/if}
</div>

<style>
  .editor {
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
  }
  .frame {
    position: relative;
    display: flex;
    flex: 1;
    min-height: 0;
    font: 12.5px/20px var(--font-mono);
    background: var(--code-bg);
    border-top: 1px solid var(--line);
    border-bottom: 1px solid var(--line);
  }
  .gutter {
    flex: none;
    overflow: hidden;
    padding: 8px 0;
    text-align: right;
    color: var(--muted);
    user-select: none;
    border-right: 1px solid var(--line);
  }
  .ln {
    display: flex;
    height: 20px;
    align-items: center;
    justify-content: flex-end;
    gap: 4px;
    padding: 0 8px 0 6px;
  }
  .ln.hl {
    color: var(--fg);
  }
  .num {
    min-width: 2ch;
    font-variant-numeric: tabular-nums;
  }
  .text {
    position: relative;
    flex: 1;
    min-width: 0;
  }
  .band {
    position: absolute;
    left: 0;
    right: 0;
    height: 20px;
    background: color-mix(in srgb, var(--accent) 16%, transparent);
    box-shadow: inset 2px 0 0 var(--accent);
    pointer-events: none;
  }
  .overlay,
  textarea {
    margin: 0;
    padding: 8px;
    white-space: pre;
    tab-size: 2;
    font: inherit;
  }
  .overlay {
    position: absolute;
    inset: 0;
    overflow: hidden;
    pointer-events: none;
    color: var(--fg);
  }
  textarea {
    position: relative;
    display: block;
    width: 100%;
    height: 100%;
    box-sizing: border-box;
    resize: none;
    overflow: auto;
    background: transparent;
    color: transparent;
    caret-color: var(--fg);
    border: 0;
    outline: none;
  }
  textarea::selection {
    background: color-mix(in srgb, var(--accent) 35%, transparent);
  }
  .measure {
    position: absolute;
    visibility: hidden;
    white-space: pre;
  }
  .t-comment {
    color: var(--muted);
    font-style: italic;
  }
  .t-string {
    color: var(--code-str);
  }
  .t-number {
    color: var(--code-num);
  }
  .t-keyword {
    color: var(--code-key);
  }
  .t-node {
    color: var(--fg);
    font-weight: 600;
  }
  .t-property {
    color: var(--code-prop);
  }
  .t-punct {
    color: var(--muted);
  }
  .sq-error {
    text-decoration: underline wavy var(--danger);
  }
  .sq-warning {
    text-decoration: underline wavy var(--warn);
  }
  .mark.error {
    color: var(--danger);
  }
  .mark.warning {
    color: var(--warn);
  }
  .suggest {
    position: absolute;
    z-index: 10;
    max-height: 14rem;
    min-width: 14rem;
    overflow: auto;
    margin: 0;
    padding: 4px 0;
    list-style: none;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 8px;
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.25);
    font: 13px var(--font-ui);
  }
  .suggest li {
    display: flex;
    align-items: baseline;
    gap: 12px;
    padding: 2px 8px;
    cursor: pointer;
  }
  .suggest li[aria-selected='true'] {
    background: var(--raised);
  }
  .label {
    font-family: var(--font-mono);
  }
  .detail {
    color: var(--muted);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .help {
    margin: 6px 12px;
    color: var(--muted);
    font-size: 12px;
  }
  .problems {
    padding: 0 12px 8px;
  }
  .problems h3 {
    margin: 4px 0;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--muted);
  }
  .problems ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .problems button {
    display: flex;
    width: 100%;
    gap: 8px;
    align-items: baseline;
    text-align: left;
    background: transparent;
    border: 0;
    border-radius: 6px;
    padding: 3px 4px;
    color: var(--fg);
    cursor: pointer;
  }
  .problems button:hover {
    background: var(--raised);
  }
  .at {
    font: 12px var(--font-mono);
    color: var(--muted);
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
    white-space: nowrap;
  }
</style>
