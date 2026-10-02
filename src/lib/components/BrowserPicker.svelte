<script lang="ts">
  import { onMount, tick, untrack } from "svelte";
  import * as api from "$lib/api";
  import { app, errorMessage } from "$lib/state.svelte";
  import { describeTarget, findOption, optionKey, targetOptions, toTarget } from "$lib/targets";
  import type { Bookmark, LaunchTarget, ResolvedTarget } from "$lib/types";
  import { ui } from "$lib/ui.svelte";

  interface Props {
    bookmark: Bookmark;
    /** The bookmark's default browser, preselected. */
    resolved: ResolvedTarget | null;
    /** E.g. why opening the default directly failed. */
    initialError?: string | null;
    onclose: () => void;
  }

  let { bookmark, resolved: resolvedProp, initialError = null, onclose }: Props = $props();

  type Remember = "" | "bookmark" | "folder";

  const options = $derived(targetOptions(app.browsers ?? []));
  // Seeded from props; "Forget" clears it locally.
  let resolved = $state(untrack(() => resolvedProp));
  let selected = $state(0);
  let privateMode = $state(untrack(() => resolvedProp?.target.private ?? false));
  let remember = $state<Remember>("");
  let busy = $state(false);
  let loading = $state(false);
  let error = $state<string | null>(untrack(() => initialError));
  let list = $state<HTMLElement>();

  const folderName = $derived(bookmark.folderId === null ? null : app.folderPath(bookmark.folderId));
  const rememberChoices = $derived<Remember[]>(folderName ? ["", "bookmark", "folder"] : ["", "bookmark"]);

  onMount(async () => {
    try {
      const [last] = await Promise.all([api.getLastTarget(), app.browsers ? null : load(false)]);
      const preselect = resolved?.target ?? last;
      if (preselect) selected = Math.max(findOption(options, preselect), 0);
    } catch (e) {
      error = errorMessage(e);
    }
  });

  async function load(refresh: boolean) {
    loading = true;
    try {
      await app.loadBrowsers(refresh);
      selected = Math.min(selected, Math.max(options.length - 1, 0));
    } finally {
      loading = false;
    }
  }

  async function refresh() {
    error = null;
    try {
      await load(true);
    } catch (e) {
      error = errorMessage(e);
    }
  }

  /** Takes the bookmark explicitly: it runs after `onclose()`, when this component's props
   * are gone (the parent's `ui.picking` is null by then). */
  async function saveDefault(b: Bookmark, target: LaunchTarget, where: "bookmark" | "folder") {
    if (where === "bookmark") await api.updateBookmark(b.id, { defaultTarget: target });
    else if (b.folderId !== null) await api.setFolderTarget(b.folderId, target);
    await app.reloadAll();
  }

  async function open(index: number, privateWindow: boolean) {
    const option = options[index];
    if (!option || busy) return;
    selected = index;
    if (privateWindow && !option.browser.supportsPrivate) {
      error = `${option.browser.name} has no private mode.`;
      return;
    }
    const target = toTarget(option, privateWindow);
    busy = true;
    error = null;
    try {
      await api.openUrl(bookmark.id, target);
    } catch (e) {
      error = errorMessage(e);
      busy = false;
      return;
    }
    // Read everything needed before closing; props aren't readable afterwards.
    const where = remember;
    const opened = bookmark;
    onclose();
    // The link is already open; a failure to save the default goes to the main error banner.
    if (where) await app.run(() => saveDefault(opened, target, where));
  }

  /** Clears the default where it is set: the bookmark, or the (possibly ancestor) folder. */
  async function forget() {
    if (!resolved) return;
    const { folderId } = resolved;
    try {
      if (folderId === null) await api.updateBookmark(bookmark.id, { defaultTarget: null });
      else await api.setFolderTarget(folderId, null);
      await app.reloadAll();
      resolved = null;
    } catch (e) {
      error = errorMessage(e);
    }
  }

  async function move(delta: number) {
    if (options.length === 0) return;
    selected = (selected + delta + options.length) % options.length;
    await tick();
    list?.querySelector(".option.selected")?.scrollIntoView({ block: "nearest" });
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    // Let form controls (the Remember select) handle their own keys.
    if (e.target instanceof HTMLSelectElement) return;
    // `code`, not `key`: Shift+1 reports key "!".
    const digit = /^(?:Digit|Numpad)([1-9])$/.exec(e.code);
    if (digit) {
      e.preventDefault();
      open(Number(digit[1]) - 1, privateMode || e.shiftKey);
    } else if (e.key === "Enter") {
      e.preventDefault();
      open(selected, privateMode || e.shiftKey);
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      move(1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      move(-1);
    } else if (e.key === "p" || e.key === "P") {
      e.preventDefault();
      privateMode = !privateMode;
    } else if (e.key === "r" || e.key === "R") {
      e.preventDefault();
      remember = rememberChoices[(rememberChoices.indexOf(remember) + 1) % rememberChoices.length];
    } else if (e.key === "Escape") {
      e.preventDefault();
      onclose();
    }
  }

  function initial(name: string) {
    return name.trim().charAt(0).toUpperCase() || "?";
  }
</script>

<svelte:window {onkeydown} />

<div class="backdrop" role="dialog" aria-modal="true" aria-labelledby="picker-title">
  <div class="dialog">
    <header>
      <h2 id="picker-title">Open with…</h2>
      <div class="url" title={bookmark.url}>{bookmark.title || bookmark.url}</div>
    </header>

    {#if resolved}
      <div class="current">
        <span>
          Default: <strong>{describeTarget(resolved.target, app.browsers)}</strong>
          <span class="source">
            {resolved.folderName ? `from folder “${resolved.folderName}”` : "for this bookmark"}
          </span>
        </span>
        <button class="link" onclick={forget}>Forget</button>
      </div>
    {/if}

    {#if app.browsers === null}
      <p class="status">Looking for browsers…</p>
    {:else if options.length === 0}
      <p class="status">No browsers to show. Use Refresh, or Manage… to add or unhide one.</p>
    {:else}
      <div class="options" bind:this={list} role="listbox" aria-label="Browsers">
        {#each options as option, i (optionKey(option))}
          <div
            class="option"
            class:selected={i === selected}
            role="option"
            aria-selected={i === selected}
            tabindex="-1"
            onmouseenter={() => (selected = i)}
          >
            <button class="main" disabled={busy} onclick={() => open(i, privateMode)}>
              <span class="key">{i < 9 ? i + 1 : ""}</span>
              <span class="badge {option.browser.kind}" aria-hidden="true">{initial(option.browser.name)}</span>
              <span class="label">
                <span class="name">{option.browser.name}</span>
                {#if option.profile}
                  <span class="profile">
                    {option.profile.name}{#if option.profile.email}<span class="email">
                        · {option.profile.email}</span
                      >{/if}
                  </span>
                {/if}
              </span>
            </button>
            {#if option.browser.supportsPrivate}
              <button
                class="private"
                disabled={busy}
                title="Open in a private window (Shift+{i < 9 ? i + 1 : 'Enter'})"
                onclick={() => open(i, true)}>Private</button
              >
            {/if}
          </div>
        {/each}
      </div>
    {/if}

    {#if error}
      <p class="error" role="alert">{error}</p>
    {/if}

    <footer>
      <div class="controls">
        <label class="toggle">
          <input type="checkbox" bind:checked={privateMode} />
          Private window <kbd>P</kbd>
        </label>
        <label class="toggle">
          <select bind:value={remember} aria-label="Remember this choice">
            <option value="">Don't remember</option>
            <option value="bookmark">Remember for this bookmark</option>
            {#if folderName}
              <option value="folder">Remember for “{folderName}”</option>
            {/if}
          </select>
          <kbd>R</kbd>
        </label>
      </div>
      <div class="controls">
        <label class="toggle" title="When a bookmark has a default browser, open it without showing this picker. Shift+click shows the picker anyway.">
          <input
            type="checkbox"
            checked={app.openDirect}
            onchange={(e) => app.run(() => app.setOpenDirect(e.currentTarget.checked))}
          />
          Skip this picker when a default is set
        </label>
        <span class="hint">1–9/Enter open · Shift private · Esc cancel</span>
        <button onclick={refresh} disabled={loading} title="Scan for installed browsers again">
          {loading ? "Scanning…" : "Refresh"}
        </button>
        <button
          title="Hide, reorder, rename or add browsers"
          onclick={() => {
            onclose();
            ui.browsersOpen = true;
          }}>Manage…</button
        >
      </div>
    </footer>
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
    width: min(540px, calc(100vw - 32px));
    max-height: calc(100vh - 48px);
    padding: 18px;
    border-radius: 10px;
    background: var(--bg);
    box-shadow: 0 10px 40px rgb(0 0 0 / 0.3);
  }

  h2 {
    margin: 0;
    font-size: 16px;
  }

  .url {
    overflow: hidden;
    color: var(--muted);
    font-size: 12px;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .status {
    margin: 12px 0;
    color: var(--muted);
    text-align: center;
  }

  .options {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0 -6px;
    overflow-y: auto;
  }

  .option {
    display: flex;
    align-items: center;
    gap: 6px;
    padding-right: 6px;
    border-radius: 6px;
    outline: none;
  }

  .option.selected {
    background: var(--selected);
  }

  .main {
    display: flex;
    flex: 1;
    align-items: center;
    gap: 10px;
    min-width: 0;
    padding: 6px;
    border: none;
    text-align: left;
  }

  .main:hover:not(:disabled) {
    background: transparent;
  }

  .key {
    width: 12px;
    color: var(--muted);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    text-align: right;
  }

  .badge {
    flex: none;
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 13px;
    font-weight: 600;
  }

  .badge.firefox {
    background: rgb(255 120 0 / 0.16);
    color: #e66000;
  }

  .badge.safari,
  .badge.other {
    background: var(--hover);
    color: var(--muted);
  }

  .label {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .name,
  .profile {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .name {
    font-weight: 500;
  }

  .profile {
    color: var(--muted);
    font-size: 12px;
  }

  .private {
    padding: 2px 8px;
    font-size: 12px;
    visibility: hidden;
  }

  .option.selected .private {
    visibility: visible;
  }

  .error {
    margin: 0;
  }

  .current {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 6px 10px;
    border-radius: 6px;
    background: var(--hover);
    font-size: 12px;
  }

  .source {
    color: var(--muted);
  }

  .link {
    padding: 0;
    border: none;
    color: var(--accent);
    font-size: 12px;
  }

  .link:hover:not(:disabled) {
    background: none;
    text-decoration: underline;
  }

  footer {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding-top: 8px;
    border-top: 1px solid var(--border);
    font-size: 12px;
  }

  .controls {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .controls select {
    padding: 2px 4px;
    font-size: 12px;
  }

  .toggle {
    display: flex;
    align-items: center;
    gap: 6px;
    white-space: nowrap;
  }

  .toggle input {
    margin: 0;
  }

  kbd {
    padding: 0 4px;
    border: 1px solid var(--border);
    border-radius: 3px;
    color: var(--muted);
    font: inherit;
    font-size: 11px;
  }

  .hint {
    flex: 1;
    color: var(--muted);
  }

  footer button {
    padding: 3px 10px;
    font-size: 12px;
  }
</style>
