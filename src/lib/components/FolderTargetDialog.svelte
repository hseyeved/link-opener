<script lang="ts">
  import { untrack } from "svelte";
  import * as api from "$lib/api";
  import { app, errorMessage } from "$lib/state.svelte";
  import { describeTarget } from "$lib/targets";
  import type { Folder, LaunchTarget } from "$lib/types";
  import TargetSelect from "./TargetSelect.svelte";

  let { folder, onclose }: { folder: Folder; onclose: () => void } = $props();

  let target = $state<LaunchTarget | null>(untrack(() => folder.defaultTarget));
  let saving = $state(false);
  let error = $state<string | null>(null);

  const inherited = $derived(app.inheritedTarget(folder.parentId));
  const emptyLabel = $derived(
    inherited
      ? `Inherit: ${describeTarget(inherited.target, app.browsers)} (from ${inherited.folder.name})`
      : "Ask every time",
  );

  async function save() {
    if (saving) return;
    saving = true;
    error = null;
    try {
      await api.setFolderTarget(folder.id, target);
      await app.reloadFolders();
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
    } else if (e.key === "Enter" && !(e.target instanceof HTMLSelectElement)) {
      e.preventDefault();
      save();
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="backdrop" role="dialog" aria-modal="true" aria-labelledby="folder-target-title">
  <div class="dialog">
    <h2 id="folder-target-title">Default browser for “{folder.name}”</h2>
    <p class="explain">
      Bookmarks in this folder and its subfolders open with this browser, unless they have their own default.
    </p>
    <TargetSelect bind:value={target} {emptyLabel} />
    {#if error}
      <p class="error" role="alert">{error}</p>
    {/if}
    <div class="buttons">
      <button type="button" onclick={onclose}>Cancel</button>
      <button type="button" class="primary" disabled={saving} onclick={save}>Save</button>
    </div>
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
    width: min(480px, calc(100vw - 32px));
    padding: 20px;
    border-radius: 10px;
    background: var(--bg);
    box-shadow: 0 10px 40px rgb(0 0 0 / 0.3);
  }

  h2 {
    margin: 0;
    font-size: 16px;
  }

  .explain {
    margin: 0;
    color: var(--muted);
    font-size: 13px;
  }

  .error {
    margin: 0;
  }

  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
