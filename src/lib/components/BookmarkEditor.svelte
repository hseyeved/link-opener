<script lang="ts">
  import { untrack } from "svelte";
  import { focus } from "$lib/actions";
  import * as api from "$lib/api";
  import { app, errorMessage } from "$lib/state.svelte";
  import { describeTarget } from "$lib/targets";
  import type { Bookmark, LaunchTarget } from "$lib/types";
  import Favicon from "./Favicon.svelte";
  import TagInput from "./TagInput.svelte";
  import TargetSelect from "./TargetSelect.svelte";

  interface Props {
    /** `null` to add a new bookmark. */
    bookmark: Bookmark | null;
    /** Folder for a new bookmark. */
    folderId: number | null;
    onclose: () => void;
  }

  let { bookmark, folderId, onclose }: Props = $props();

  // The editor is mounted per edit, so the props only seed the form.
  const initial = untrack(() => ({ bookmark, folderId }));
  const isNew = initial.bookmark === null;
  let title = $state(initial.bookmark?.title ?? "");
  let url = $state(initial.bookmark?.url ?? "");
  let notes = $state(initial.bookmark?.notes ?? "");
  let folder = $state<number | null>(initial.bookmark ? initial.bookmark.folderId : initial.folderId);
  let tags = $state<string[]>(initial.bookmark?.tags ?? []);
  let pendingTag = $state("");
  let target = $state<LaunchTarget | null>(initial.bookmark?.defaultTarget ?? null);
  let favicon = $state<string | null>(initial.bookmark?.favicon ?? null);
  let saving = $state(false);
  let error = $state<string | null>(null);

  // Title/favicon fetching: automatic (debounced) when the URL changes, or on demand.
  let fetching = $state(false);
  let fetchNote = $state<string | null>(null);
  let fetchSeq = 0;

  /** Worth fetching: a host-looking string, not a half-typed one. */
  function fetchable(u: string): boolean {
    const v = u.trim();
    return (
      v.length > 3 &&
      !/\s/.test(v) &&
      (v.includes(".") || v.includes("localhost")) &&
      !/^(mailto|tel|data|about|file):/i.test(v)
    );
  }

  /** "example.com/x" from "https://www.example.com/x", for the letter fallback icon. */
  function hostPart(u: string): string {
    return u.trim().replace(/^[a-z][\w+.-]*:\/\/(www\.)?/i, "");
  }

  /** `replaceTitle`: the manual button overwrites; auto-fetch only fills an empty title. */
  async function fetchMeta(replaceTitle: boolean) {
    const u = url;
    const mine = ++fetchSeq;
    fetching = true;
    fetchNote = null;
    try {
      const meta = await api.fetchMetadata(u);
      if (mine !== fetchSeq) return;
      if (meta.title && (replaceTitle || !title.trim())) title = meta.title;
      favicon = meta.favicon;
      if (!meta.title && !meta.favicon) fetchNote = "Nothing found for this page.";
    } catch (e) {
      if (mine === fetchSeq) fetchNote = errorMessage(e);
    } finally {
      if (mine === fetchSeq) fetching = false;
    }
  }

  $effect(() => {
    const u = url;
    // Don't fetch for the URL the bookmark already had.
    if (u === untrack(() => initial.bookmark?.url ?? "") || !fetchable(u)) return;
    const timer = setTimeout(() => fetchMeta(false), 700);
    return () => clearTimeout(timer);
  });

  const canSave = $derived(url.trim() !== "" && !saving);
  // What "no default of its own" means for the chosen folder.
  const inherited = $derived(app.inheritedTarget(folder));
  const emptyTargetLabel = $derived(
    inherited
      ? `Folder default: ${describeTarget(inherited.target, app.browsers)} (from ${inherited.folder.name})`
      : "Ask every time",
  );

  async function save() {
    if (!canSave) return;
    saving = true;
    error = null;
    try {
      // Include a tag that was typed but not yet confirmed; the backend dedupes.
      const allTags = pendingTag.trim() ? [...tags, ...pendingTag.split(",")] : tags;
      const fields = { title, url, notes, tags: allTags, defaultTarget: target, favicon };
      const saved = initial.bookmark
        ? await api.updateBookmark(initial.bookmark.id, { ...fields, folderId: folder })
        : await api.createBookmark({ ...fields, folderId: folder });
      await app.reloadBookmarks();
      // Saving never waits for a fetch: if the icon (or title) hasn't arrived, finish in
      // the background and refresh the list when it does.
      if ((fetching || !saved.favicon || !saved.title) && fetchable(saved.url)) {
        api
          .refreshMetadata(saved.id)
          .then(() => app.reloadBookmarks())
          .catch(() => {});
      }
      onclose();
    } catch (e) {
      error = errorMessage(e);
    } finally {
      saving = false;
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      onclose();
    } else if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      save();
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="backdrop" role="dialog" aria-modal="true" aria-labelledby="editor-title">
  <form
    class="dialog"
    onsubmit={(e) => {
      e.preventDefault();
      save();
    }}
  >
    <h2 id="editor-title">{isNew ? "Add bookmark" : "Edit bookmark"}</h2>

    <div class="field">
      <span>URL</span>
      <div class="url-row">
        <Favicon {favicon} fallback={hostPart(url) || "?"} />
        {#if isNew}
          <input bind:value={url} placeholder="example.com" spellcheck="false" aria-label="URL" use:focus />
        {:else}
          <input bind:value={url} placeholder="example.com" spellcheck="false" aria-label="URL" />
        {/if}
        <button
          type="button"
          class="icon"
          title="Fetch title and icon from the page"
          aria-label="Fetch title and icon"
          disabled={fetching || !fetchable(url)}
          onclick={() => fetchMeta(true)}>{fetching ? "…" : "↻"}</button
        >
      </div>
      {#if fetching}
        <small class="note">Fetching title and icon…</small>
      {:else if fetchNote}
        <small class="note">{fetchNote}</small>
      {/if}
    </div>

    <label>
      <span>Title</span>
      {#if isNew}
        <input bind:value={title} placeholder="Optional" />
      {:else}
        <input bind:value={title} placeholder="Optional" use:focus />
      {/if}
    </label>

    <label>
      <span>Notes</span>
      <textarea bind:value={notes} rows="3"></textarea>
    </label>

    <label>
      <span>Folder</span>
      <select bind:value={folder}>
        <option value={null}>{app.unfiledName}</option>
        {#each app.flatFolders as { folder: f, depth } (f.id)}
          <option value={f.id}>{"   ".repeat(depth)}{f.name}</option>
        {/each}
      </select>
    </label>

    <div class="field">
      <span>Tags</span>
      <TagInput bind:tags bind:pending={pendingTag} />
    </div>

    <div class="field">
      <span>Default browser</span>
      <TargetSelect bind:value={target} emptyLabel={emptyTargetLabel} />
    </div>

    {#if error}
      <p class="error" role="alert">{error}</p>
    {/if}

    <div class="buttons">
      <span class="hint">Ctrl+Enter to save · Esc to cancel</span>
      <button type="button" onclick={onclose}>Cancel</button>
      <button type="submit" class="primary" disabled={!canSave}>Save</button>
    </div>
  </form>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    background: rgb(0 0 0 / 0.35);
    z-index: 10;
  }

  .dialog {
    display: flex;
    flex-direction: column;
    gap: 12px;
    width: min(520px, calc(100vw - 32px));
    max-height: calc(100vh - 32px);
    overflow-y: auto;
    padding: 20px;
    border-radius: 10px;
    background: var(--bg);
    box-shadow: 0 10px 40px rgb(0 0 0 / 0.3);
  }

  h2 {
    margin: 0 0 4px;
    font-size: 16px;
  }

  label,
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  label > span,
  .field > span {
    color: var(--muted);
    font-size: 12px;
  }

  textarea {
    resize: vertical;
  }

  .url-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .url-row input {
    flex: 1;
    min-width: 0;
  }

  .note {
    color: var(--muted);
    font-size: 12px;
  }

  .buttons {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 4px;
  }

  .hint {
    flex: 1;
    color: var(--muted);
    font-size: 12px;
  }

  .error {
    margin: 0;
  }
</style>
