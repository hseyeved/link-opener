<script lang="ts">
  import { tick } from "svelte";
  import { focus } from "$lib/actions";
  import * as api from "$lib/api";
  import { app, errorMessage } from "$lib/state.svelte";
  import type { Bookmark } from "$lib/types";
  import { ui } from "$lib/ui.svelte";
  import Favicon from "./Favicon.svelte";

  function close() {
    ui.searching = false;
  }

  /** Enter: open (with its default browser, or the picker). Shift: always the picker. */
  function open(bookmark: Bookmark, forcePicker: boolean) {
    close();
    ui.open(bookmark, forcePicker);
  }

  let query = $state("");
  let results = $state<Bookmark[]>([]);
  let selected = $state(0);
  let error = $state<string | null>(null);
  let list = $state<HTMLElement>();

  const terms = $derived(query.toLowerCase().split(/\s+/).filter(Boolean));

  // Debounced search; `seq` drops responses that arrive after a newer query.
  let seq = 0;
  $effect(() => {
    const q = query;
    const mine = ++seq;
    const timer = setTimeout(
      async () => {
        try {
          const found = await api.search(q, 50);
          if (mine !== seq) return;
          results = found;
          selected = 0;
          error = null;
        } catch (e) {
          if (mine === seq) error = errorMessage(e);
        }
      },
      q.trim() ? 80 : 0,
    );
    return () => clearTimeout(timer);
  });

  async function move(delta: number) {
    if (results.length === 0) return;
    selected = (selected + delta + results.length) % results.length;
    await tick();
    list?.querySelector(".result.selected")?.scrollIntoView({ block: "nearest" });
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      move(1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      move(-1);
    } else if (e.key === "Enter") {
      e.preventDefault();
      const bookmark = results[selected];
      if (!bookmark) return;
      if (e.ctrlKey || e.metaKey) {
        close();
        ui.editing = { bookmark };
      } else {
        open(bookmark, e.shiftKey);
      }
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "m") {
      e.preventDefault();
      const bookmark = results[selected];
      if (!bookmark) return;
      close();
      ui.moving = { kind: "bookmark", bookmark };
    } else if (e.key === "Escape") {
      e.preventDefault();
      close();
    }
  }

  function host(url: string): string {
    try {
      return new URL(url).host || url;
    } catch {
      return url;
    }
  }

  /** Splits `text` into runs, marking those that match any search term. */
  function highlight(text: string): { text: string; match: boolean }[] {
    const lower = text.toLowerCase();
    const marked = new Array<boolean>(text.length).fill(false);
    for (const term of terms) {
      for (let i = lower.indexOf(term); i !== -1; i = lower.indexOf(term, i + 1)) {
        marked.fill(true, i, i + term.length);
      }
    }
    const runs: { text: string; match: boolean }[] = [];
    for (let i = 0; i < text.length; i++) {
      const last = runs.at(-1);
      if (last && last.match === marked[i]) last.text += text[i];
      else runs.push({ text: text[i], match: marked[i] });
    }
    return runs;
  }
</script>

{#snippet marked(text: string)}
  {#each highlight(text) as run, i (i)}{#if run.match}<mark>{run.text}</mark>{:else}{run.text}{/if}{/each}
{/snippet}

<svelte:window {onkeydown} />

<div class="backdrop" role="dialog" aria-modal="true" aria-label="Search bookmarks">
  <div class="palette">
    <input
      class="query"
      bind:value={query}
      placeholder="Search bookmarks…"
      spellcheck="false"
      autocomplete="off"
      aria-label="Search"
      use:focus
    />

    {#if error}
      <p class="error" role="alert">{error}</p>
    {:else if results.length === 0}
      <p class="status">{query.trim() ? "No matches." : "No bookmarks yet."}</p>
    {:else}
      {#if !query.trim()}
        <div class="section">Recent</div>
      {/if}
      <div class="results" bind:this={list} role="listbox" aria-label="Results">
        {#each results as b, i (b.id)}
          {@const h = host(b.url)}
          <!-- Keyboard users stay in the input and use ↑/↓/Enter; clicking is the mouse path. -->
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <div
            class="result"
            class:selected={i === selected}
            role="option"
            aria-selected={i === selected}
            tabindex="-1"
            onmouseenter={() => (selected = i)}
            onclick={(e) => open(b, e.shiftKey)}
          >
            <Favicon favicon={b.favicon} fallback={h} />
            <div class="body">
              <div class="title">{@render marked(b.title || h)}</div>
              <div class="meta">
                <span class="url">{@render marked(b.url)}</span>
                {#each b.tags as tag (tag)}
                  <span class="tag">#{@render marked(tag)}</span>
                {/each}
                <span class="folder">{app.folderPath(b.folderId)}</span>
              </div>
            </div>
          </div>
        {/each}
      </div>
    {/if}

    <footer>
      <span><kbd>↑</kbd><kbd>↓</kbd> select</span>
      <span><kbd>Enter</kbd> open</span>
      <span><kbd>Shift</kbd>+<kbd>Enter</kbd> choose browser</span>
      <span><kbd>Ctrl</kbd>+<kbd>Enter</kbd> edit</span>
      <span><kbd>Ctrl</kbd>+<kbd>M</kbd> move</span>
      <span><kbd>Esc</kbd> close</span>
    </footer>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 12vh;
    background: rgb(0 0 0 / 0.35);
    z-index: 10;
  }

  .palette {
    display: flex;
    flex-direction: column;
    width: min(620px, calc(100vw - 32px));
    max-height: 70vh;
    border-radius: 10px;
    background: var(--bg);
    box-shadow: 0 10px 40px rgb(0 0 0 / 0.3);
    overflow: hidden;
  }

  .query {
    padding: 14px 16px;
    border: none;
    border-bottom: 1px solid var(--border);
    border-radius: 0;
    background: transparent;
    font-size: 16px;
  }

  .query:focus {
    outline: none;
  }

  .status,
  .error {
    margin: 12px 16px;
  }

  .status {
    color: var(--muted);
  }

  .section {
    padding: 8px 16px 2px;
    color: var(--muted);
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .results {
    padding: 4px 6px;
    overflow-y: auto;
  }

  .result {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 10px;
    border-radius: 6px;
    cursor: pointer;
    outline: none;
  }

  .result.selected {
    background: var(--selected);
  }


  .body {
    flex: 1;
    min-width: 0;
  }

  .title,
  .meta {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .title {
    font-weight: 500;
  }

  .meta {
    display: flex;
    gap: 8px;
    color: var(--muted);
    font-size: 12px;
  }

  .url {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .tag {
    flex: none;
    color: var(--accent);
  }

  .folder {
    flex: none;
    max-width: 40%;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  mark {
    border-radius: 2px;
    background: rgb(255 200 0 / 0.35);
    color: inherit;
  }

  footer {
    display: flex;
    gap: 14px;
    padding: 8px 16px;
    border-top: 1px solid var(--border);
    color: var(--muted);
    font-size: 11px;
  }

  kbd {
    margin: 0 1px;
    padding: 0 4px;
    border: 1px solid var(--border);
    border-radius: 3px;
    font: inherit;
  }
</style>
