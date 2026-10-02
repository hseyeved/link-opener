<script lang="ts" module>
  export interface MenuItem {
    label: string;
    onselect: () => void;
    /** Text shown after the first click; a second click runs the item. Shown in red. */
    confirm?: string;
  }
</script>

<script lang="ts">
  import { tick } from "svelte";

  let { items, label = "More actions" }: { items: MenuItem[]; label?: string } = $props();

  let open = $state(false);
  let armed = $state<number | null>(null);
  let trigger = $state<HTMLButtonElement>();
  let menu = $state<HTMLElement>();
  // Fixed position, so the sidebar's scroll container doesn't clip the menu.
  let position = $state({ top: 0, left: 0 });

  async function toggle() {
    if (open) return close();
    const rect = trigger!.getBoundingClientRect();
    position = { top: rect.bottom + 2, left: rect.left };
    open = true;
    armed = null;
    await tick();
    if (menu) {
      // Keep the menu inside the window.
      const width = menu.offsetWidth;
      position.left = Math.min(position.left, window.innerWidth - width - 8);
      menu.querySelector("button")?.focus();
    }
  }

  function close() {
    open = false;
    armed = null;
  }

  function select(item: MenuItem, i: number) {
    if (item.confirm && armed !== i) {
      armed = i;
      return;
    }
    close();
    item.onselect();
  }

  function onwindowclick(e: MouseEvent) {
    if (open && !menu?.contains(e.target as Node) && !trigger?.contains(e.target as Node)) close();
  }

  function onkeydown(e: KeyboardEvent) {
    if (!open) return;
    const buttons = [...(menu?.querySelectorAll("button") ?? [])];
    const i = buttons.indexOf(document.activeElement as HTMLButtonElement);
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      close();
      trigger?.focus();
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const next = (i + (e.key === "ArrowDown" ? 1 : -1) + buttons.length) % buttons.length;
      buttons[next]?.focus();
    }
  }
</script>

<svelte:window onclick={onwindowclick} />

<button bind:this={trigger} class="icon" title={label} aria-label={label} aria-haspopup="menu" aria-expanded={open} onclick={toggle}
  >⋯</button
>

{#if open}
  <div
    bind:this={menu}
    class="menu"
    role="menu"
    tabindex="-1"
    style:top="{position.top}px"
    style:left="{position.left}px"
    {onkeydown}
  >
    {#each items as item, i (item.label)}
      <button role="menuitem" class:danger={!!item.confirm} onclick={() => select(item, i)}>
        {armed === i ? item.confirm : item.label}
      </button>
    {/each}
  </div>
{/if}

<style>
  .menu {
    position: fixed;
    z-index: 20;
    display: flex;
    flex-direction: column;
    min-width: 170px;
    padding: 4px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--bg);
    box-shadow: 0 6px 24px rgb(0 0 0 / 0.2);
  }

  .menu button {
    padding: 6px 10px;
    border: none;
    border-radius: 5px;
    font-size: 13px;
    text-align: left;
  }

  .menu button:focus-visible {
    outline: none;
    background: var(--hover);
  }

  .menu button.danger {
    color: var(--danger);
  }
</style>
