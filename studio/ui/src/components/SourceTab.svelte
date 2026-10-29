<!--
  The Source tab (docs/studio-interface.md, Source): the file the open
  definition is written in, its problems marked, the definition's line
  highlighted; or the file of a place opened from the problems list
  (#70), at that place. Typing edits the library as any gesture does: each change
  is sent as one splice, grouped into undo steps by word and pause.
-->
<script lang="ts">
  import SourceEditor from './SourceEditor.svelte';
  import { splice } from '../lib/diff';
  import { Typist } from '../lib/typing';
  import type { Studio } from '../lib/state.svelte';
  import type { Problem } from '../lib/isolex/syntax';
  import { byteOf, lineOf, rowAt } from '../lib/select';

  interface Props {
    studio: Studio;
  }

  let { studio }: Props = $props();

  const file = $derived(studio.place?.file ?? studio.definition?.file ?? studio.view.library?.files[0] ?? null);
  let editor: SourceEditor | undefined = $state();
  /** The place last revealed, so each is revealed once, its text loaded. */
  let revealed: object | null = null;
  $effect(() => {
    const p = studio.place;
    void loaded;
    if (!p || p === revealed || !editor || shown !== p.file || !text) return;
    revealed = p;
    void editor.reveal({ line: p.line, col: p.col, message: p.message, severity: p.severity });
  });
  let text = $state('');
  /** What the core holds, as far as this tab knows: edits are made
      against it. */
  let known = '';
  let shown: string | null = null;
  let sending: Promise<unknown> = Promise.resolve();
  /** Counts texts loaded, for what waits on one. */
  let loaded = $state(0);

  const typist = new Typist({
    begin: () => void studio.backend.beginGesture(),
    end: () => void studio.backend.endGesture(),
  });

  /** The file's text again whenever the library changes, unless it is what
      this tab already has. */
  $effect(() => {
    const f = file;
    void studio.view.library?.revision;
    if (!f) return;
    void studio.backend.source(f).then((src) => {
      if (!src || f !== file) return;
      if (f !== shown || src.text !== known) {
        shown = f;
        known = src.text;
        if (src.text !== text) text = src.text;
        loaded++;
      }
    });
  });

  function onedit(value: string) {
    if (!file) return;
    const change = splice(known, value);
    if (!change) return;
    known = value;
    const f = file;
    typist.before();
    sending = sending.then(async () => {
      const answer = await studio.backend.ask({ kind: 'type', file: f, ...change }, `Type in ${f}`);
      if (answer.kind === 'refused') studio.message = answer.reason;
    });
    typist.after(change.text);
  }

  const problems = $derived.by((): Problem[] => {
    const c = studio.view.compiled;
    if (!c || !file) return [];
    const mine = (d: { file: string }) => d.file === file;
    return [
      ...c.errors.filter(mine).map((d) => ({ line: d.line, col: d.col, message: d.message, severity: 'error' as const })),
      ...c.warnings.filter(mine).map((d) => ({ line: d.line, col: d.col, message: d.message, severity: 'warning' as const })),
    ];
  });

  /** The selection's first line, or the open definition's. */
  const highlight = $derived.by(() => {
    const at = studio.selected ?? studio.editingNode;
    return at && at.file === file ? lineOf(text, at.start) : null;
  });

  /** A caret put inside a node of the open definition selects it. */
  function oncaret(offset: number) {
    if (!file) return;
    const f = rowAt(studio.outline, file, byteOf(text, offset));
    if (f) studio.select(f.row!.node);
  }

  const vocabulary = $derived({
    tokens: Object.keys(studio.view.palette ?? {}),
    definitions: [],
    icons: studio.view.compiled?.icons ?? [],
  });
</script>

{#if file}
  <div class="source">
    <div class="head"><span class="file">{file}</span><span class="note">recompiles as you type</span></div>
    <div class="body" onfocusout={() => typist.close()}>
      <SourceEditor bind:this={editor} bind:value={text} {problems} {vocabulary} {highlight} label="Source of {file}" {onedit} {oncaret} />
    </div>
  </div>
{:else}
  <p class="empty">Open a library to see its source.</p>
{/if}

<style>
  .source {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
  .head {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    padding: 8px 12px;
    color: var(--muted);
    font-size: 12px;
  }
  .file {
    font-family: var(--font-mono);
    color: var(--fg);
  }
  .body {
    flex: 1;
    min-height: 0;
  }
  .empty {
    margin: 12px;
    color: var(--muted);
  }
</style>
