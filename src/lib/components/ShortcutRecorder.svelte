<script lang="ts">
  interface Props {
    /** Current shortcut in Tauri's format ("Ctrl+Alt+Space"); "" = none. */
    value: string;
    /** Called with a newly recorded shortcut. */
    onrecord: (shortcut: string) => void;
    disabled?: boolean;
  }

  let { value, onrecord, disabled = false }: Props = $props();

  const isMac = navigator.userAgent.includes("Mac");
  let recording = $state(false);
  let held = $state("");

  const MODIFIER_CODES = /^(Control|Alt|Shift|Meta|OS)(Left|Right)?$/;

  function modifiers(e: KeyboardEvent): string[] {
    const mods: string[] = [];
    if (e.ctrlKey) mods.push("Ctrl");
    if (e.altKey) mods.push("Alt");
    if (e.shiftKey) mods.push("Shift");
    if (e.metaKey) mods.push(isMac ? "Cmd" : "Super");
    return mods;
  }

  /** `KeyK` → "K", `Digit1` → "1"; other codes ("Space", "F5", "ArrowUp") as is. */
  function keyName(code: string): string {
    return code.replace(/^Key([A-Z])$/, "$1").replace(/^Digit(\d)$/, "$1");
  }

  function onkeydown(e: KeyboardEvent) {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape" && !e.ctrlKey && !e.altKey && !e.metaKey) {
      recording = false;
      return;
    }
    const mods = modifiers(e);
    if (MODIFIER_CODES.test(e.code)) {
      // Only modifiers so far: show them and keep waiting for the key.
      held = mods.join("+");
      return;
    }
    recording = false;
    onrecord([...mods, keyName(e.code)].join("+"));
  }

  /** "Ctrl+Alt+Space" → ["Ctrl", "Alt", "Space"] for display. */
  const keys = $derived(value ? value.split("+") : []);
</script>

<button
  type="button"
  class="recorder"
  class:recording
  {disabled}
  onmousedown={(e) => e.preventDefault()}
  onclick={(e) => {
    // WebKit (macOS) doesn't focus buttons on click, and the key events only reach a focused
    // button; mousedown is cancelled so a second click toggles off instead of blurring first.
    e.currentTarget.focus();
    recording = !recording;
    held = "";
  }}
  {onkeydown}
  onblur={() => (recording = false)}
  title={recording ? "Press the new shortcut (Esc to cancel)" : "Click to record a new shortcut"}
>
  {#if recording}
    <span class="prompt">{held ? `${held}+…` : "Press keys…"}</span>
  {:else if keys.length}
    {#each keys as key, i (i)}
      <kbd>{key}</kbd>
    {/each}
  {:else}
    <span class="prompt">None</span>
  {/if}
</button>

<style>
  .recorder {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    min-width: 160px;
    min-height: 32px;
    padding: 4px 10px;
  }

  .recorder.recording {
    border-color: var(--accent);
    outline: 2px solid var(--accent-soft);
  }

  .prompt {
    color: var(--muted);
  }

  kbd {
    padding: 1px 6px;
    border: 1px solid var(--border);
    border-bottom-width: 2px;
    border-radius: 4px;
    font: inherit;
    font-size: 12px;
  }
</style>
