<script lang="ts">
  import { onMount } from "svelte";
  import * as api from "$lib/api";
  import { app, errorMessage } from "$lib/state.svelte";
  import type { Browser, BrowserKind, BrowserPrefs } from "$lib/types";
  import InlineInput from "./InlineInput.svelte";

  let { onclose }: { onclose: () => void } = $props();

  let prefs = $state<BrowserPrefs | null>(null);
  let busy = $state(false);
  let scanning = $state(false);
  let error = $state<string | null>(null);
  /** Key ("<browser>" or "<browser>/<profile>") being renamed. */
  let renaming = $state<string | null>(null);

  let newName = $state("");
  let newPath = $state("");
  let newKind = $state<BrowserKind>("chromium");

  const browsers = $derived(app.browsers ?? []);
  const KIND_LABELS: Record<BrowserKind, string> = {
    chromium: "Chromium-based",
    firefox: "Firefox-based",
    safari: "Safari",
    other: "Other (URL only)",
  };

  onMount(async () => {
    try {
      const [loaded] = await Promise.all([api.getBrowserPrefs(), app.browsers ? null : app.loadBrowsers()]);
      prefs = loaded;
    } catch (e) {
      error = errorMessage(e);
    }
  });

  /** Saves `next`; on failure the previous prefs stay. Returns success. */
  async function save(next: BrowserPrefs): Promise<boolean> {
    busy = true;
    error = null;
    try {
      app.browsers = await api.setBrowserPrefs(next);
      // Re-read: new custom browsers get their ids on save.
      prefs = await api.getBrowserPrefs();
      return true;
    } catch (e) {
      error = errorMessage(e);
      return false;
    } finally {
      busy = false;
    }
  }

  function edit(change: (p: BrowserPrefs) => void) {
    if (!prefs) return;
    const next = $state.snapshot(prefs) as BrowserPrefs;
    change(next);
    save(next);
  }

  function setHidden(key: string, hidden: boolean) {
    edit((p) => {
      p.hidden = p.hidden.filter((k) => k !== key);
      if (hidden) p.hidden.push(key);
    });
  }

  function rename(key: string, defaultName: string, name: string) {
    renaming = null;
    edit((p) => {
      // Renaming back to the detected name just removes the label.
      if (name === defaultName) delete p.labels[key];
      else p.labels[key] = name;
    });
  }

  function move(index: number, delta: number) {
    const order = browsers.map((b) => b.id);
    const to = index + delta;
    if (to < 0 || to >= order.length) return;
    [order[index], order[to]] = [order[to], order[index]];
    edit((p) => (p.order = order));
  }

  function removeCustom(browser: Browser) {
    edit((p) => {
      p.custom = p.custom.filter((c) => c.id !== browser.id);
      const own = (k: string) => k === browser.id || k.startsWith(`${browser.id}/`);
      p.order = p.order.filter((k) => !own(k));
      p.hidden = p.hidden.filter((k) => !own(k));
      for (const k of Object.keys(p.labels)) if (own(k)) delete p.labels[k];
    });
  }

  async function addCustom(e: SubmitEvent) {
    e.preventDefault();
    if (!prefs) return;
    const next = $state.snapshot(prefs) as BrowserPrefs;
    next.custom.push({ id: "", name: newName, path: newPath, kind: newKind });
    if (await save(next)) {
      newName = "";
      newPath = "";
    }
  }

  function resetPrefs() {
    // Keeps custom browsers; clears order, hiding and names.
    edit((p) => {
      p.order = [];
      p.hidden = [];
      p.labels = {};
    });
  }

  async function rescan() {
    scanning = true;
    error = null;
    try {
      await app.loadBrowsers(true);
    } catch (e) {
      error = errorMessage(e);
    } finally {
      scanning = false;
    }
  }

  function onkeydown(e: KeyboardEvent) {
    // Escape inside an inline rename only cancels the rename (InlineInput stops it).
    if (e.key === "Escape") {
      e.preventDefault();
      onclose();
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="backdrop" role="dialog" aria-modal="true" aria-labelledby="browsers-title">
  <div class="dialog">
    <header>
      <h2 id="browsers-title">Browsers</h2>
      <div class="header-actions">
        <button onclick={rescan} disabled={scanning} title="Scan for installed browsers and profiles again">
          {scanning ? "Scanning…" : "Rescan"}
        </button>
        <button class="icon" aria-label="Close" onclick={onclose}>✕</button>
      </div>
    </header>
    <p class="muted">
      Untick to hide from the picker. The order sets the picker's 1–9 keys. Double-click a name to rename it.
    </p>

    {#if !prefs}
      <p class="muted">{error ?? "Loading…"}</p>
    {:else}
      <ul class="browsers">
        {#each browsers as browser, i (browser.id)}
          <li class:hidden={browser.hidden}>
            <div class="row">
              <input
                type="checkbox"
                checked={!browser.hidden}
                disabled={busy}
                aria-label="Show {browser.name}"
                onchange={(e) => setHidden(browser.id, !e.currentTarget.checked)}
              />
              <div class="label">
                {#if renaming === browser.id}
                  <InlineInput
                    value={browser.name}
                    onsubmit={(name) => rename(browser.id, browser.defaultName, name)}
                    oncancel={() => (renaming = null)}
                  />
                {:else}
                  <button class="name" ondblclick={() => (renaming = browser.id)} title="Double-click to rename">
                    {browser.name}
                  </button>
                  {#if browser.name !== browser.defaultName}
                    <span class="muted">({browser.defaultName})</span>
                  {/if}
                {/if}
                <span class="meta" title={browser.path}>
                  {KIND_LABELS[browser.kind]}{browser.custom ? " · added by you" : ""} · {browser.path}
                </span>
              </div>
              <div class="actions">
                <button class="icon" aria-label="Rename {browser.name}" title="Rename" onclick={() => (renaming = browser.id)}
                  >✎</button
                >
                <button
                  class="icon"
                  aria-label="Move {browser.name} up"
                  title="Move up"
                  disabled={busy || i === 0}
                  onclick={() => move(i, -1)}>↑</button
                >
                <button
                  class="icon"
                  aria-label="Move {browser.name} down"
                  title="Move down"
                  disabled={busy || i === browsers.length - 1}
                  onclick={() => move(i, 1)}>↓</button
                >
                {#if browser.custom}
                  <button class="icon" aria-label="Remove {browser.name}" title="Remove" onclick={() => removeCustom(browser)}
                    >✕</button
                  >
                {/if}
              </div>
            </div>

            {#if browser.profiles.length > 1}
              <ul class="profiles">
                {#each browser.profiles as profile (profile.id)}
                  {@const key = `${browser.id}/${profile.id}`}
                  <li class:hidden={profile.hidden}>
                    <input
                      type="checkbox"
                      checked={!profile.hidden}
                      disabled={busy || browser.hidden}
                      aria-label="Show {browser.name} profile {profile.name}"
                      onchange={(e) => setHidden(key, !e.currentTarget.checked)}
                    />
                    {#if renaming === key}
                      <InlineInput
                        value={profile.name}
                        onsubmit={(name) => rename(key, profile.defaultName, name)}
                        oncancel={() => (renaming = null)}
                      />
                    {:else}
                      <button class="name" ondblclick={() => (renaming = key)} title="Double-click to rename">
                        {profile.name}
                      </button>
                      {#if profile.name !== profile.defaultName}
                        <span class="muted">({profile.defaultName})</span>
                      {/if}
                      {#if profile.email}
                        <span class="muted">· {profile.email}</span>
                      {/if}
                    {/if}
                  </li>
                {/each}
              </ul>
            {/if}
          </li>
        {:else}
          <li class="muted">No browsers found. Try Rescan, or add one below.</li>
        {/each}
      </ul>

      <form class="add" onsubmit={addCustom}>
        <h3>Add a browser</h3>
        <p class="muted">For browsers the scan doesn't find, such as portable installs.</p>
        <div class="add-row">
          <input bind:value={newName} placeholder="Name" aria-label="Browser name" />
          <select bind:value={newKind} aria-label="Browser type">
            <option value="chromium">{KIND_LABELS.chromium}</option>
            <option value="firefox">{KIND_LABELS.firefox}</option>
            <option value="other">{KIND_LABELS.other}</option>
          </select>
        </div>
        <div class="add-row">
          <input
            class="path"
            bind:value={newPath}
            placeholder={navigator.userAgent.includes("Mac")
              ? "/Applications/Browser.app"
              : navigator.userAgent.includes("Windows")
                ? "C:\\Path\\to\\browser.exe"
                : "/path/to/browser"}
            aria-label="Path to the browser"
            spellcheck="false"
          />
          <button type="submit" class="primary" disabled={busy || !newName.trim() || !newPath.trim()}>Add</button>
        </div>
      </form>

      {#if error}
        <p class="error" role="alert">{error}</p>
      {/if}

      <footer>
        <button disabled={busy} onclick={resetPrefs}>Reset names, order and visibility</button>
      </footer>
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
    width: min(620px, calc(100vw - 32px));
    max-height: calc(100vh - 32px);
    overflow-y: auto;
    padding: 20px;
    border-radius: 10px;
    background: var(--bg);
    box-shadow: 0 10px 40px rgb(0 0 0 / 0.3);
  }

  header,
  .header-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  header {
    justify-content: space-between;
  }

  h2 {
    margin: 0;
    font-size: 16px;
  }

  h3 {
    margin: 0;
    font-size: 13px;
  }

  .muted {
    margin: 0;
    color: var(--muted);
    font-size: 12px;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .browsers > li {
    padding: 8px 0;
    border-top: 1px solid var(--border);
  }

  .row {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .label {
    display: flex;
    flex: 1;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0 6px;
    min-width: 0;
  }

  .name {
    padding: 0;
    border: none;
    font-weight: 500;
    text-align: left;
  }

  .name:hover:not(:disabled) {
    background: none;
  }

  .meta {
    flex-basis: 100%;
    overflow: hidden;
    color: var(--muted);
    font-size: 11px;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .hidden .name,
  .hidden .meta {
    opacity: 0.5;
  }

  .actions {
    display: flex;
    gap: 2px;
  }

  .profiles {
    margin: 4px 0 0 26px;
  }

  .profiles li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 2px 0;
    font-size: 13px;
  }

  .profiles .name {
    font-weight: normal;
  }

  .add {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding-top: 12px;
    border-top: 1px solid var(--border);
  }

  .add-row {
    display: flex;
    gap: 8px;
  }

  .add-row input {
    flex: 1;
    min-width: 0;
  }

  .path {
    font-family: ui-monospace, monospace;
    font-size: 12px;
  }

  .error {
    margin: 0;
  }

  footer {
    display: flex;
    justify-content: flex-end;
  }
</style>
