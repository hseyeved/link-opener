<script lang="ts">
  import { untrack } from "svelte";
  import { focus } from "$lib/actions";

  interface Props {
    value?: string;
    placeholder?: string;
    /** Called with the trimmed text when it is non-empty and changed. */
    onsubmit: (value: string) => void;
    oncancel: () => void;
  }

  let { value = "", placeholder = "", onsubmit, oncancel }: Props = $props();

  let text = $state(untrack(() => value));
  // Enter is followed by a blur when the input unmounts; finish only once.
  let done = false;

  function commit() {
    if (done) return;
    done = true;
    const v = text.trim();
    if (v && v !== value) onsubmit(v);
    else oncancel();
  }

  function cancel() {
    if (done) return;
    done = true;
    oncancel();
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      commit();
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      cancel();
    }
  }
</script>

<input class="inline-input" bind:value={text} {placeholder} use:focus {onkeydown} onblur={commit} />

<style>
  .inline-input {
    width: 100%;
    min-width: 0;
    padding: 2px 6px;
    font: inherit;
  }
</style>
