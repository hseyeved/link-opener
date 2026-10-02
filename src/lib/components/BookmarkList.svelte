<script lang="ts">
  import * as api from "$lib/api";
  import { dnd, pointerFraction } from "$lib/dnd.svelte";
  import { app } from "$lib/state.svelte";
  import { describeTarget } from "$lib/targets";
  import type { Bookmark } from "$lib/types";
  import { ui } from "$lib/ui.svelte";
  import Favicon from "./Favicon.svelte";
  import ConfirmButton from "./ConfirmButton.svelte";

  /** Row under the pointer while reordering, and which half of it. */
  let dropAt = $state<{ id: number; after: boolean } | null>(null);

  function ondblclick(e: MouseEvent, b: Bookmark) {
    // Double-clicking the title button would also open the picker twice; leave buttons alone.
    if ((e.target as Element).closest("button")) return;
    ui.editing = { bookmark: b };
  }

  function host(url: string): string {
    try {
      return new URL(url).host || url;
    } catch {
      return url;
    }
  }

  async function remove(id: number) {
    await app.run(async () => {
      await api.deleteBookmark(id);
      await app.reloadBookmarks();
    });
  }

  function ondragstart(e: DragEvent, b: Bookmark) {
    dnd.item = { kind: "bookmark", id: b.id };
    if (e.dataTransfer) {
      // Dropping on a browser or another app gives it the link.
      e.dataTransfer.setData("text/uri-list", b.url);
      e.dataTransfer.setData("text/plain", b.url);
      e.dataTransfer.effectAllowed = "copyMove";
    }
  }

  function ondragover(e: DragEvent, b: Bookmark) {
    if (dnd.item?.kind !== "bookmark" || dnd.item.id === b.id) {
      dropAt = null;
      return;
    }
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
    dropAt = { id: b.id, after: pointerFraction(e) > 0.5 };
  }

  async function ondrop(e: DragEvent, b: Bookmark) {
    const item = dnd.item;
    const after = pointerFraction(e) > 0.5;
    dropAt = null;
    dnd.item = null;
    if (item?.kind !== "bookmark" || item.id === b.id) return;
    e.preventDefault();
    // Index among the folder's bookmarks, not counting the dragged one.
    const others = app.bookmarks.filter((x) => x.id !== item.id);
    const index = others.findIndex((x) => x.id === b.id) + (after ? 1 : 0);
    await app.run(async () => {
      await api.moveBookmark(item.id, app.selectedFolderId, index);
      await app.reloadBookmarks();
    });
  }
</script>

{#if app.bookmarks.length === 0}
  <p class="empty">No bookmarks here yet.</p>
{:else}
  <ul class="bookmarks">
    {#each app.bookmarks as b (b.id)}
      {@const h = host(b.url)}
      <!-- Double-click and dragging are mouse shortcuts; the Edit and Move buttons are the keyboard path. -->
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <li
        class="bookmark"
        class:drop-before={dropAt?.id === b.id && !dropAt.after}
        class:drop-after={dropAt?.id === b.id && dropAt.after}
        draggable="true"
        ondragstart={(e) => ondragstart(e, b)}
        ondragend={() => {
          dnd.item = null;
          dropAt = null;
        }}
        ondragover={(e) => ondragover(e, b)}
        ondragleave={() => (dropAt = null)}
        ondrop={(e) => ondrop(e, b)}
        ondblclick={(e) => ondblclick(e, b)}
      >
        <Favicon favicon={b.favicon} fallback={h} />
        <div class="body">
          <button
            class="title"
            title="Open {b.url} (Shift+click to choose the browser)"
            onclick={(e) => ui.open(b, e.shiftKey)}>{b.title || h}</button
          >
          <div class="meta">
            <span class="host" title={b.url}>{h}</span>
            {#if b.defaultTarget}
              <span class="target" title="Default browser">↗ {describeTarget(b.defaultTarget, app.browsers)}</span>
            {/if}
            {#if b.notes.trim()}
              <span class="notes">{b.notes.trim().split("\n")[0]}</span>
            {/if}
          </div>
        </div>
        {#if b.tags.length}
          <span class="tags">
            {#each b.tags as tag (tag)}
              <span class="tag">{tag}</span>
            {/each}
          </span>
        {/if}
        <span class="actions">
          <button class="icon" title="Edit" aria-label="Edit" onclick={() => (ui.editing = { bookmark: b })}>✎</button>
          <button
            class="icon"
            title="Move to…"
            aria-label="Move to…"
            onclick={() => (ui.moving = { kind: "bookmark", bookmark: b })}>⇄</button
          >
          <ConfirmButton label="✕" title="Delete bookmark" onconfirm={() => remove(b.id)} />
        </span>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .empty {
    padding: 32px;
    color: var(--muted);
    text-align: center;
  }

  .bookmarks {
    list-style: none;
    margin: 0;
    padding: 4px 8px;
  }

  .bookmark {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border-radius: 6px;
    cursor: default;
    user-select: none;
  }

  .bookmark:hover {
    background: var(--hover);
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
    display: block;
    max-width: 100%;
    padding: 0;
    border: none;
    font-weight: 500;
    text-align: left;
  }

  .title:hover:not(:disabled) {
    background: none;
    color: var(--accent);
    text-decoration: underline;
  }

  .meta {
    color: var(--muted);
    font-size: 12px;
  }

  .notes::before,
  .target::before {
    content: "·";
    margin: 0 6px;
  }

  .target {
    color: var(--accent);
  }

  .tags {
    display: flex;
    flex: none;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 4px;
    max-width: 35%;
    overflow: hidden;
    max-height: 22px;
  }

  .tag {
    padding: 1px 8px;
    border-radius: 10px;
    background: var(--hover);
    color: var(--muted);
    font-size: 11px;
    white-space: nowrap;
  }

  .actions {
    display: flex;
    gap: 2px;
    visibility: hidden;
  }

  .bookmark:hover .actions,
  .bookmark:focus-within .actions {
    visibility: visible;
  }
</style>
