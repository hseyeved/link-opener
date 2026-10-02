<script lang="ts">
  import { onMount } from "svelte";
  import BookmarkEditor from "$lib/components/BookmarkEditor.svelte";
  import BookmarkList from "$lib/components/BookmarkList.svelte";
  import BrowserManager from "$lib/components/BrowserManager.svelte";
  import BrowserPicker from "$lib/components/BrowserPicker.svelte";
  import CommandPalette from "$lib/components/CommandPalette.svelte";
  import FolderTargetDialog from "$lib/components/FolderTargetDialog.svelte";
  import FolderTree from "$lib/components/FolderTree.svelte";
  import Menu from "$lib/components/Menu.svelte";
  import MoveDialog from "$lib/components/MoveDialog.svelte";
  import ImportDialog from "$lib/components/ImportDialog.svelte";
  import Settings from "$lib/components/Settings.svelte";
  import * as api from "$lib/api";
  import { backUp, count, exportHtml, startImport } from "$lib/data";
  import { app } from "$lib/state.svelte";
  import { ui } from "$lib/ui.svelte";

  const searchShortcut = navigator.userAgent.includes("Mac") ? "⌘K" : "Ctrl K";

  async function fetchMissingIcons() {
    ui.notify("Fetching icons…", 120_000);
    const ok = await app.run(async () => {
      const found = await api.fetchMissingFavicons();
      await app.reloadBookmarks();
      ui.notify(`Fetched ${count(found, "icon")}.`);
    });
    if (!ok) ui.notify(null);
  }

  const toolbarMenu = [
    { label: "Import…", onselect: startImport },
    { label: "Back up…", onselect: backUp },
    { label: "Export for browsers…", onselect: exportHtml },
    { label: "Fetch missing icons", onselect: fetchMissingIcons },
    { label: "Browsers…", onselect: () => (ui.browsersOpen = true) },
    { label: "Settings…", onselect: () => (ui.settingsOpen = true) },
  ];

  function onkeydown(e: KeyboardEvent) {
    if (!(e.ctrlKey || e.metaKey) || e.altKey || e.shiftKey) return;
    // Ctrl/Cmd+K toggles search, unless another dialog is open.
    if (e.key.toLowerCase() === "k") {
      if (ui.dialogOpen && !ui.searching) return;
      e.preventDefault();
      ui.searching = !ui.searching;
    } else if (e.key === ",") {
      if (ui.dialogOpen) return;
      e.preventDefault();
      ui.settingsOpen = true;
    }
  }

  onMount(() => {
    app.init();
    // Detect browsers in the background so the picker opens instantly; it retries on failure.
    app.loadBrowsers().catch(() => {});
    // The global shortcut and tray menu ask for a view; it replaces whatever dialog is open.
    const unlisten = api.onOpenView((view) => {
      ui.closeAll();
      if (view === "search") ui.searching = true;
      else ui.settingsOpen = true;
    });
    return () => {
      unlisten.then((stop) => stop());
    };
  });
</script>

<svelte:window {onkeydown} />

<div class="app">
  <aside class="sidebar">
    <FolderTree />
  </aside>

  <main class="main">
    <header class="toolbar">
      <h1>{app.selectedFolder?.name ?? app.unfiledName}</h1>
      {#if ui.notice}
        <span class="notice" role="status">{ui.notice}</span>
      {/if}
      <div class="toolbar-actions">
        <button class="search" onclick={() => (ui.searching = true)} title="Search bookmarks">
          Search <kbd>{searchShortcut}</kbd>
        </button>
        <button class="primary" onclick={() => (ui.editing = { bookmark: null })}>Add bookmark</button>
        <Menu items={toolbarMenu} />
      </div>
    </header>

    {#if app.error}
      <div class="error banner" role="alert">
        <span>{app.error}</span>
        <button class="icon" aria-label="Dismiss" onclick={() => (app.error = null)}>✕</button>
      </div>
    {/if}

    <div class="content">
      <BookmarkList />
    </div>
  </main>
</div>

{#if ui.searching}
  <CommandPalette />
{/if}

{#if ui.picking}
  <BrowserPicker
    bookmark={ui.picking.bookmark}
    resolved={ui.picking.resolved}
    initialError={ui.picking.error}
    onclose={() => (ui.picking = null)}
  />
{/if}

{#if ui.editing}
  <BookmarkEditor bookmark={ui.editing.bookmark} folderId={app.selectedFolderId} onclose={() => (ui.editing = null)} />
{/if}

{#if ui.moving}
  <MoveDialog item={ui.moving} onclose={() => (ui.moving = null)} />
{/if}

{#if ui.importing}
  <ImportDialog path={ui.importing.path} preview={ui.importing.preview} onclose={() => (ui.importing = null)} />
{/if}

{#if ui.browsersOpen}
  <BrowserManager onclose={() => (ui.browsersOpen = false)} />
{/if}

{#if ui.settingsOpen}
  <Settings onclose={() => (ui.settingsOpen = false)} />
{/if}

{#if ui.folderTarget}
  <FolderTargetDialog folder={ui.folderTarget} onclose={() => (ui.folderTarget = null)} />
{/if}

<style>
  .app {
    display: grid;
    grid-template-columns: 240px 1fr;
    height: 100vh;
  }

  .sidebar {
    overflow-y: auto;
    border-right: 1px solid var(--border);
    background: var(--bg-sidebar);
  }

  .main {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 12px 18px;
    border-bottom: 1px solid var(--border);
  }

  h1 {
    margin: 0;
    overflow: hidden;
    font-size: 16px;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .notice {
    flex: 1;
    color: var(--muted);
    font-size: 12px;
    text-align: right;
  }

  .toolbar-actions {
    display: flex;
    flex: none;
    align-items: center;
    gap: 8px;
  }

  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--muted);
  }

  .search kbd {
    padding: 0 4px;
    border: 1px solid var(--border);
    border-radius: 3px;
    font: inherit;
    font-size: 11px;
  }

  .banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin: 8px 18px 0;
  }

  .content {
    flex: 1;
    overflow-y: auto;
  }
</style>
