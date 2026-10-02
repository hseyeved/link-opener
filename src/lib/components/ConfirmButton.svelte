<script lang="ts">
  import { focus } from "$lib/actions";

  interface Props {
    label: string;
    title: string;
    onconfirm: () => void;
  }

  let { label, title, onconfirm }: Props = $props();

  // First click arms the button, second click confirms; losing focus disarms it.
  let armed = $state(false);
</script>

{#if armed}
  <button
    class="danger"
    title="Click again to confirm"
    use:focus
    onclick={() => {
      armed = false;
      onconfirm();
    }}
    onblur={() => (armed = false)}>Confirm?</button
  >
{:else}
  <button class="icon" {title} aria-label={title} onclick={() => (armed = true)}>{label}</button>
{/if}
