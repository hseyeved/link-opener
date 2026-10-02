<script lang="ts">
  import * as api from "$lib/api";
  import { dnd, pointerFraction, type DropZone } from "$lib/dnd.svelte";
  import { app, type FolderNode } from "$lib/state.svelte";
  import { describeTarget } from "$lib/targets";
  import { ui } from "$lib/ui.svelte";
  import InlineInput from "./InlineInput.svelte";
  import Menu from "./Menu.svelte";
  import Self from "./FolderNode.svelte";

  let { node, depth }: { node: FolderNode; depth: number } = $props();

  let expanded = $state(true);
  let mode = $state<"rename" | "add" | null>(null);
  let zone = $state<DropZone | null>(null);

  // Depth 0 lines up with the "Unfiled" row above the tree; each level adds 14px.
  const indent = (d: number) => `${d * 14}px`;

  const menuItems = $derived([
    { label: "Rename", onselect: () => (mode = "rename") },
    { label: "Move to…", onselect: () => (ui.moving = { kind: "folder", folder: node }) },
    { label: "Default browser…", onselect: () => (ui.folderTarget = node) },
    { label: "Delete", confirm: "Delete folder and contents?", onselect: remove },
  ]);

  async function rename(name: string) {
    mode = null;
    await app.run(async () => {
      await api.renameFolder(node.id, name);
      await app.reloadFolders();
    });
  }

  async function addChild(name: string) {
    mode = null;
    await app.run(async () => {
      await api.createFolder(node.id, name);
      await app.reloadFolders();
    });
  }

  async function remove() {
    await app.run(async () => {
      await api.deleteFolder(node.id);
      await app.reloadAll();
    });
  }

  /** Bookmarks drop into the folder. Folders drop before/after it (top/bottom quarter) or into it. */
  function zoneFor(e: DragEvent): DropZone | null {
    const item = dnd.item;
    if (!item) return null;
    if (item.kind === "bookmark") return "into";
    if (app.isWithin(node.id, item.id)) return null; // itself or one of its subfolders
    const y = pointerFraction(e);
    return y < 0.25 ? "before" : y > 0.75 ? "after" : "into";
  }

  function ondragstart(e: DragEvent) {
    dnd.item = { kind: "folder", id: node.id };
    e.dataTransfer?.setData("text/plain", node.name);
    if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
  }

  function ondragover(e: DragEvent) {
    zone = zoneFor(e);
    if (zone) {
      e.preventDefault();
      if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
    }
  }

  async function ondrop(e: DragEvent) {
    const item = dnd.item;
    const where = zoneFor(e);
    zone = null;
    dnd.item = null;
    if (!item || !where) return;
    e.preventDefault();
    await app.run(async () => {
      if (item.kind === "bookmark") {
        await api.moveBookmark(item.id, node.id);
      } else if (where === "into") {
        await api.moveFolder(item.id, node.id);
        expanded = true;
      } else {
        // Index among the new siblings, not counting the dragged folder.
        const siblings = app.folders.filter((f) => f.parentId === node.parentId && f.id !== item.id);
        const index = siblings.findIndex((f) => f.id === node.id) + (where === "after" ? 1 : 0);
        await api.moveFolder(item.id, node.parentId, index);
      }
      await app.reloadAll();
    });
  }
</script>

<li>
  <div
    class="row"
    class:selected={app.selectedFolderId === node.id}
    class:drop-before={zone === "before"}
    class:drop-after={zone === "after"}
    class:drop-into={zone === "into"}
    style:padding-left={indent(depth)}
    draggable={mode === null}
    role="treeitem"
    aria-selected={app.selectedFolderId === node.id}
    tabindex="-1"
    {ondragstart}
    ondragend={() => {
      dnd.item = null;
      zone = null;
    }}
    {ondragover}
    ondragleave={() => (zone = null)}
    {ondrop}
  >
    {#if node.children.length > 0}
      <button
        class="twisty"
        class:expanded
        aria-label={expanded ? "Collapse" : "Expand"}
        onclick={() => (expanded = !expanded)}>▸</button
      >
    {:else}
      <span class="twisty"></span>
    {/if}

    {#if mode === "rename"}
      <InlineInput value={node.name} onsubmit={rename} oncancel={() => (mode = null)} />
    {:else}
      <button
        class="name"
        title={node.defaultTarget
          ? `${node.name} — opens in ${describeTarget(node.defaultTarget, app.browsers)}`
          : node.name}
        onclick={() => app.run(() => app.select(node.id))}
        ondblclick={() => (mode = "rename")}
        >{node.name}{#if node.defaultTarget}<span class="has-target" aria-label="has a default browser">
            ●</span
          >{/if}</button
      >
      <span class="actions">
        <button
          class="icon"
          title="Add subfolder"
          aria-label="Add subfolder"
          onclick={() => {
            expanded = true;
            mode = "add";
          }}>+</button
        >
        <Menu items={menuItems} label="Folder actions" />
      </span>
    {/if}
  </div>

  {#if expanded && (node.children.length > 0 || mode === "add")}
    <ul role="group">
      {#each node.children as child (child.id)}
        <Self node={child} depth={depth + 1} />
      {/each}
      {#if mode === "add"}
        <li class="new-row" style:padding-left={indent(depth + 1)}>
          <span class="twisty"></span>
          <InlineInput placeholder="Subfolder name" onsubmit={addChild} oncancel={() => (mode = null)} />
        </li>
      {/if}
    </ul>
  {/if}
</li>

<style>
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .new-row {
    display: flex;
    align-items: center;
    padding-top: 2px;
    padding-bottom: 2px;
  }

  .has-target {
    margin-left: 4px;
    color: var(--accent);
    font-size: 8px;
    vertical-align: middle;
  }
</style>
