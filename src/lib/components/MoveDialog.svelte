<script lang="ts">
  import { tick } from "svelte";
  import { focus } from "$lib/actions";
  import * as api from "$lib/api";
  import { app, errorMessage } from "$lib/state.svelte";
  import type { MoveItem } from "$lib/ui.svelte";

  let { item, onclose }: { item: MoveItem; onclose: () => void } = $props();

  interface Destination {
    /** `null` = Unfiled (bookmarks) or the top level (folders). */
    id: number | null;
    path: string;
  }

  let query = $state("");
  let selected = $state(0);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let list = $state<HTMLElement>();

  const name = $derived(item.kind === "bookmark" ? item.bookmark.title || item.bookmark.url : item.folder.name);
  const current = $derived(item.kind === "bookmark" ? item.bookmark.folderId : item.folder.parentId);

  const destinations = $derived.by((): Destination[] => {
    const root: Destination = {
      id: null,
      path: item.kind === "bookmark" ? app.unfiledName : "Top level",
    };
    const folders = app.flatFolders
      .map(({ folder }) => folder)
      // A folder can't move into itself or its own subfolders.
      .filter((f) => item.kind === "bookmark" || !app.isWithin(f.id, item.folder.id))
      .map((f) => ({ id: f.id, path: app.folderPath(f.id) }));
    const q = query.trim().toLowerCase();
    return [root, ...folders].filter((d) => !q || d.path.toLowerCase().includes(q));
  });

  $effect(() => {
    // Keep the selection in range as the filter changes.
    if (selected >= destinations.length) selected = Math.max(destinations.length - 1, 0);
  });

  async function moveTo(destination: Destination | undefined) {
    if (!destination || busy) return;
    busy = true;
    error = null;
    try {
      if (item.kind === "bookmark") await api.moveBookmark(item.bookmark.id, destination.id);
      else await api.moveFolder(item.folder.id, destination.id);
      await app.reloadAll();
      onclose();
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  }

  async function step(delta: number) {
    if (destinations.length === 0) return;
    selected = (selected + delta + destinations.length) % destinations.length;
    await tick();
    list?.querySelector(".destination.selected")?.scrollIntoView({ block: "nearest" });
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      step(1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      step(-1);
    } else if (e.key === "Enter") {
      e.preventDefault();
      moveTo(destinations[selected]);
    } else if (e.key === "Escape") {
      e.preventDefault();
      onclose();
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="backdrop" role="dialog" aria-modal="true" aria-label="Move {name}">
  <div class="palette">
    <div class="heading">Move <strong>{name}</strong> to…</div>
    <input
      class="query"
      bind:value={query}
      oninput={() => (selected = 0)}
      placeholder="Filter folders…"
      spellcheck="false"
      autocomplete="off"
      aria-label="Filter folders"
      use:focus
    />

    {#if destinations.length === 0}
      <p class="status">No matching folders.</p>
    {:else}
      <div class="results" bind:this={list} role="listbox" aria-label="Folders">
        {#each destinations as d, i (d.id)}
          <!-- Keyboard users stay in the input and use ↑/↓/Enter; clicking is the mouse path. -->
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <div
            class="destination"
            class:selected={i === selected}
            role="option"
            aria-selected={i === selected}
            tabindex="-1"
            onmouseenter={() => (selected = i)}
            onclick={() => moveTo(d)}
          >
            <span class="path">{d.path}</span>
            {#if d.id === current}
              <span class="tag">current</span>
            {/if}
          </div>
        {/each}
      </div>
    {/if}

    {#if error}
      <p class="error" role="alert">{error}</p>
    {/if}
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
    width: min(520px, calc(100vw - 32px));
    max-height: 70vh;
    border-radius: 10px;
    background: var(--bg);
    box-shadow: 0 10px 40px rgb(0 0 0 / 0.3);
    overflow: hidden;
  }

  .heading {
    padding: 12px 16px 0;
    overflow: hidden;
    color: var(--muted);
    font-size: 12px;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .heading strong {
    color: var(--fg);
  }

  .query {
    padding: 10px 16px 12px;
    border: none;
    border-bottom: 1px solid var(--border);
    border-radius: 0;
    background: transparent;
    font-size: 15px;
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

  .results {
    padding: 4px 6px;
    overflow-y: auto;
  }

  .destination {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 10px;
    border-radius: 6px;
    cursor: pointer;
    outline: none;
  }

  .destination.selected {
    background: var(--selected);
  }

  .path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .tag {
    color: var(--muted);
    font-size: 11px;
  }
</style>
