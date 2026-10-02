<script lang="ts">
  import { app } from "$lib/state.svelte";

  interface Props {
    /** Stored file name, or `null` for the letter fallback. */
    favicon: string | null;
    /** Text whose first letter is the fallback (usually the host). */
    fallback: string;
    size?: number;
  }

  let { favicon, fallback, size = 24 }: Props = $props();

  const src = $derived(app.faviconSrc(favicon));
  // Reset when the icon changes, so a new icon gets a chance after an earlier one failed.
  let failed = $derived(src ? false : true);
</script>

{#if src && !failed}
  <img class="favicon" {src} alt="" width={size} height={size} onerror={() => (failed = true)} />
{:else}
  <span class="favicon letter" style:width="{size}px" style:height="{size}px" aria-hidden="true">
    {fallback.trim().charAt(0).toUpperCase() || "?"}
  </span>
{/if}

<style>
  .favicon {
    flex: none;
    border-radius: 5px;
  }

  img.favicon {
    object-fit: contain;
  }

  .letter {
    display: grid;
    place-items: center;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 12px;
    font-weight: 600;
  }
</style>
