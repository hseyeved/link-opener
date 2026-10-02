<script lang="ts">
  import { onMount } from "svelte";
  import * as api from "$lib/api";
  import { app, errorMessage } from "$lib/state.svelte";
  import type { DesktopSettings } from "$lib/types";
  import { ui } from "$lib/ui.svelte";
  import ShortcutRecorder from "./ShortcutRecorder.svelte";

  let { onclose }: { onclose: () => void } = $props();

  const DEFAULT_SHORTCUT = "Ctrl+Alt+Space";

  let settings = $state<DesktopSettings | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let shortcutError = $state<string | null>(null);

  onMount(async () => {
    try {
      settings = await api.getDesktopSettings();
      shortcutError = settings.shortcutError;
    } catch (e) {
      error = errorMessage(e);
    }
  });

  async function setShortcut(shortcut: string) {
    busy = true;
    shortcutError = null;
    try {
      settings = await api.setGlobalShortcut(shortcut);
    } catch (e) {
      // The previous shortcut stays active.
      shortcutError = errorMessage(e);
    } finally {
      busy = false;
    }
  }

  async function toggle(change: () => Promise<void>, revert: () => void) {
    error = null;
    try {
      await change();
    } catch (e) {
      revert();
      error = errorMessage(e);
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      onclose();
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="backdrop" role="dialog" aria-modal="true" aria-labelledby="settings-title">
  <div class="dialog">
    <header>
      <h2 id="settings-title">Settings</h2>
      <button class="icon" aria-label="Close" onclick={onclose}>✕</button>
    </header>

    {#if !settings}
      <p class="muted">{error ?? "Loading…"}</p>
    {:else}
      <section>
        <h3>Global shortcut</h3>
        <p class="muted">Shows Link Opener with search open, from any app.</p>
        <div class="row">
          <ShortcutRecorder value={settings.globalShortcut} onrecord={setShortcut} disabled={busy} />
          {#if settings.globalShortcut !== DEFAULT_SHORTCUT}
            <button disabled={busy} onclick={() => setShortcut(DEFAULT_SHORTCUT)}>Reset</button>
          {/if}
          {#if settings.globalShortcut}
            <button disabled={busy} onclick={() => setShortcut("")}>Turn off</button>
          {/if}
        </div>
        {#if shortcutError}
          <p class="error" role="alert">{shortcutError}</p>
        {/if}
        {#if settings.wayland}
          <div class="note">
            Global shortcuts don't work under Wayland. Add a custom shortcut in your desktop's keyboard
            settings (GNOME: Settings → Keyboard → Custom Shortcuts; KDE: System Settings → Shortcuts) that
            runs:
            <code>"{settings.executable}" --search</code>
          </div>
        {/if}
      </section>

      <section>
        <h3>Window</h3>
        <label class="check">
          <input
            type="checkbox"
            checked={settings.closeToTray}
            onchange={(e) => {
              const enabled = e.currentTarget.checked;
              settings!.closeToTray = enabled;
              toggle(
                () => api.setCloseToTray(enabled),
                () => (settings!.closeToTray = !enabled),
              );
            }}
          />
          <span>
            Keep running in the tray when the window is closed
            <small>Quit from the tray icon's menu.</small>
          </span>
        </label>
        <label class="check">
          <input
            type="checkbox"
            checked={settings.autostart}
            onchange={(e) => {
              const enabled = e.currentTarget.checked;
              settings!.autostart = enabled;
              toggle(
                () => api.setAutostart(enabled),
                () => (settings!.autostart = !enabled),
              );
            }}
          />
          <span>
            Launch at login
            <small>Starts in the tray, ready for the shortcut.</small>
          </span>
        </label>
      </section>

      <section>
        <h3>Opening links</h3>
        <div class="row">
          <button
            onclick={() => {
              onclose();
              ui.browsersOpen = true;
            }}>Manage browsers…</button
          >
          <span class="muted">Hide, reorder, rename or add browsers and profiles.</span>
        </div>
        <label class="check">
          <input
            type="checkbox"
            checked={app.openDirect}
            onchange={(e) => {
              const enabled = e.currentTarget.checked;
              toggle(
                () => app.setOpenDirect(enabled),
                () => {},
              );
            }}
          />
          <span>
            Skip the browser picker when a bookmark has a default browser
            <small>Shift+click (or Shift+Enter) shows the picker anyway.</small>
          </span>
        </label>
      </section>

      {#if error}
        <p class="error" role="alert">{error}</p>
      {/if}
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
    gap: 16px;
    width: min(540px, calc(100vw - 32px));
    max-height: calc(100vh - 32px);
    overflow-y: auto;
    padding: 20px;
    border-radius: 10px;
    background: var(--bg);
    box-shadow: 0 10px 40px rgb(0 0 0 / 0.3);
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  h2 {
    margin: 0;
    font-size: 16px;
  }

  section {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding-top: 12px;
    border-top: 1px solid var(--border);
  }

  h3 {
    margin: 0;
    font-size: 13px;
  }

  .muted,
  small {
    color: var(--muted);
    font-size: 12px;
  }

  .muted {
    margin: 0;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .check {
    display: flex;
    align-items: flex-start;
    gap: 8px;
  }

  .check input {
    margin: 3px 0 0;
  }

  .check span {
    display: flex;
    flex-direction: column;
  }

  .note {
    padding: 8px 10px;
    border-radius: 6px;
    background: var(--hover);
    font-size: 12px;
  }

  code {
    display: block;
    margin-top: 6px;
    word-break: break-all;
    user-select: all;
  }

  .error {
    margin: 0;
  }
</style>
