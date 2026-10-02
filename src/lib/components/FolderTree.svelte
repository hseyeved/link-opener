<script lang="ts">
  import * as api from "$lib/api";
  import { dnd } from "$lib/dnd.svelte";
  import { app } from "$lib/state.svelte";
  import FolderNode from "./FolderNode.svelte";
  import InlineInput from "./InlineInput.svelte";

  let adding = $state(false);
  let renamingUnfiled = $state(false);
  let dropTarget = $state(false);

  /** Bookmarks dropped on Unfiled are unfiled; folders move to the top level. */
  async function ondrop(e: DragEvent) {
    const item = dnd.item;
    dropTarget = false;
    dnd.item = null;
    if (!item) return;
    e.preventDefault();
    await app.run(async () => {
      if (item.kind === "bookmark") await api.moveBookmark(item.id, null);
      else await api.moveFolder(item.id, null);
      await app.reloadAll();
    });
  }

  async function renameUnfiled(name: string) {
    renamingUnfiled = false;
    await app.run(() => app.renameUnfiled(name));
  }

  async function create(name: string) {
    adding = false;
    await app.run(async () => {
      const folder = await api.createFolder(null, name);
      await app.reloadFolders();
      await app.select(folder.id);
    });
  }
</script>

<nav class="tree">
  <div class="tree-header">
    <span>Folders</span>
    <button class="icon" title="New folder" aria-label="New folder" onclick={() => (adding = true)}>+</button>
  </div>

  <div
    class="row"
    class:selected={app.selectedFolderId === null}
    class:drop-into={dropTarget}
    role="treeitem"
    aria-selected={app.selectedFolderId === null}
    tabindex="-1"
    ondragover={(e) => {
      if (!dnd.item) return;
      e.preventDefault();
      dropTarget = true;
    }}
    ondragleave={() => (dropTarget = false)}
    {ondrop}
  >
    <span class="twisty"></span>
    {#if renamingUnfiled}
      <InlineInput value={app.unfiledName} onsubmit={renameUnfiled} oncancel={() => (renamingUnfiled = false)} />
    {:else}
      <button
        class="name"
        title={app.unfiledName}
        onclick={() => app.run(() => app.select(null))}
        ondblclick={() => (renamingUnfiled = true)}>{app.unfiledName}</button
      >
      <span class="actions">
        <button class="icon" title="Rename" aria-label="Rename" onclick={() => (renamingUnfiled = true)}>✎</button>
      </span>
    {/if}
  </div>

  <ul role="tree">
    {#each app.tree as node (node.id)}
      <FolderNode {node} depth={0} />
    {/each}
    {#if adding}
      <li class="new-row">
        <InlineInput placeholder="Folder name" onsubmit={create} oncancel={() => (adding = false)} />
      </li>
    {/if}
  </ul>
</nav>

<style>
  .tree {
    display: flex;
    flex-direction: column;
    padding: 8px 6px;
    font-size: 13px;
  }

  .tree-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 4px 6px 8px;
    color: var(--muted);
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .new-row {
    padding: 2px 4px 2px 22px;
  }
</style>
