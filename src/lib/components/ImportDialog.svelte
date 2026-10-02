<script lang="ts">
  import * as api from "$lib/api";
  import { count } from "$lib/data";
  import { app, errorMessage } from "$lib/state.svelte";
  import type { ImportMode, ImportPreview, ImportSummary } from "$lib/types";

  let { path, preview, onclose }: { path: string; preview: ImportPreview; onclose: () => void } = $props();

  const isBackup = $derived(preview.kind === "backup");
  const fileName = $derived(path.split(/[\\/]/).pop() ?? path);

  let mode = $state<ImportMode>("merge");
  let skipDuplicates = $state(true);
  let folderName = $state(`Imported bookmarks ${new Date().toLocaleDateString()}`);
  let confirmReplace = $state(false);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let result = $state<ImportSummary | null>(null);

  async function run() {
    if (busy) return;
    if (mode === "replace" && !confirmReplace) {
      confirmReplace = true;
      return;
    }
    busy = true;
    error = null;
    try {
      result = await api.importFile(path, mode, skipDuplicates, isBackup ? null : folderName);
      // A replace can change settings (e.g. the Unfiled name) too, so reload everything.
      await app.select(null);
      await app.init();
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = false;
      confirmReplace = false;
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && !busy) {
      e.preventDefault();
      onclose();
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="backdrop" role="dialog" aria-modal="true" aria-labelledby="import-title">
  <div class="dialog">
    <h2 id="import-title">Import</h2>
    <p class="file" title={path}>{fileName}</p>

    {#if result}
      <p>
        Added {count(result.bookmarksAdded, "bookmark")} and {count(result.foldersCreated, "folder")}.
        {#if result.duplicatesSkipped}
          Skipped {count(result.duplicatesSkipped, "duplicate")}.
        {/if}
        {#if result.invalidSkipped}
          Skipped {count(result.invalidSkipped, "invalid link")}.
        {/if}
      </p>
      <div class="buttons">
        <button class="primary" onclick={onclose}>Done</button>
      </div>
    {:else}
      <p>
        {#if isBackup}
          Link Opener backup{#if preview.exportedAt}
            from {new Date(preview.exportedAt).toLocaleString()}{/if}
        {:else}
          Browser bookmarks
        {/if}
        with {count(preview.bookmarks, "bookmark")} in {count(preview.folders, "folder")}.
      </p>

      {#if isBackup}
        <fieldset>
          <label class="choice">
            <input type="radio" bind:group={mode} value="merge" onchange={() => (confirmReplace = false)} />
            <span>
              Add to my library
              <small>Folders with the same name are combined.</small>
            </span>
          </label>
          <label class="choice">
            <input type="radio" bind:group={mode} value="replace" />
            <span>
              Replace my library
              <small>
                Deletes all current folders and bookmarks first. A backup of them is saved in the app's data
                folder, under backups.
              </small>
            </span>
          </label>
        </fieldset>
      {:else}
        <label class="field">
          <span>Put them in a new folder named</span>
          <input bind:value={folderName} />
        </label>
      {/if}

      {#if mode === "merge"}
        <label class="choice">
          <input type="checkbox" bind:checked={skipDuplicates} />
          <span>Skip bookmarks whose link is already in my library</span>
        </label>
      {/if}

      {#if error}
        <p class="error" role="alert">{error}</p>
      {/if}

      <div class="buttons">
        <button onclick={onclose} disabled={busy}>Cancel</button>
        <button class={mode === "replace" ? "danger-solid" : "primary"} onclick={run} disabled={busy}>
          {#if busy}
            Importing…
          {:else if confirmReplace}
            Click again to replace everything
          {:else if mode === "replace"}
            Replace library
          {:else}
            Import
          {/if}
        </button>
      </div>
    {/if}
  </div>
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
    width: min(500px, calc(100vw - 32px));
    max-height: calc(100vh - 32px);
    overflow-y: auto;
    padding: 20px;
    border-radius: 10px;
    background: var(--bg);
    box-shadow: 0 10px 40px rgb(0 0 0 / 0.3);
  }

  h2 {
    margin: 0;
    font-size: 16px;
  }

  p {
    margin: 0;
  }

  .file {
    overflow: hidden;
    color: var(--muted);
    font-size: 12px;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  fieldset {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin: 0;
    padding: 0;
    border: none;
  }

  .choice {
    display: flex;
    align-items: flex-start;
    gap: 8px;
  }

  .choice input {
    margin: 3px 0 0;
  }

  .choice span {
    display: flex;
    flex-direction: column;
  }

  small {
    color: var(--muted);
    font-size: 12px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .field span {
    color: var(--muted);
    font-size: 12px;
  }

  .error {
    margin: 0;
  }

  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  .danger-solid {
    border-color: var(--danger);
    background: var(--danger);
    color: #fff;
  }
</style>
