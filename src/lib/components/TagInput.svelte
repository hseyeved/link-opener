<script lang="ts">
  import { onMount } from "svelte";
  import * as api from "$lib/api";
  import type { Tag } from "$lib/types";

  interface Props {
    tags: string[];
    /** Text typed but not yet turned into a tag; the editor adds it on save. */
    pending?: string;
  }

  let { tags = $bindable(), pending = $bindable("") }: Props = $props();

  const listId = $props.id();
  let known = $state<Tag[]>([]);
  const suggestions = $derived(
    known.filter((t) => !tags.some((name) => name.toLowerCase() === t.name.toLowerCase())),
  );

  onMount(async () => {
    try {
      known = await api.listTags();
    } catch {
      // Suggestions are optional.
    }
  });

  function add(raw: string) {
    let next = tags;
    for (const part of raw.split(",")) {
      const name = part.trim().replace(/^#+/, "").trim();
      if (name && !next.some((t) => t.toLowerCase() === name.toLowerCase())) next = [...next, name];
    }
    tags = next;
    pending = "";
  }

  function onkeydown(e: KeyboardEvent) {
    if ((e.key === "Enter" || e.key === ",") && !e.ctrlKey && !e.metaKey && pending.trim()) {
      e.preventDefault();
      add(pending);
    } else if (e.key === "Backspace" && pending === "" && tags.length > 0) {
      tags = tags.slice(0, -1);
    }
  }

  function oninput() {
    // Picking a datalist suggestion inserts its full name; take it as a tag right away.
    if (known.some((t) => t.name === pending)) add(pending);
  }
</script>

<div class="tag-input">
  {#each tags as tag (tag)}
    <span class="chip">
      {tag}
      <button
        type="button"
        aria-label="Remove tag {tag}"
        onclick={() => (tags = tags.filter((t) => t !== tag))}>×</button
      >
    </span>
  {/each}
  <input
    bind:value={pending}
    list={listId}
    placeholder={tags.length ? "" : "Add tags…"}
    aria-label="Add tag"
    {onkeydown}
    {oninput}
    onblur={() => add(pending)}
  />
  <datalist id={listId}>
    {#each suggestions as tag (tag.id)}
      <option value={tag.name}></option>
    {/each}
  </datalist>
</div>

<style>
  .tag-input {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
    padding: 4px 6px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--input-bg);
  }

  .tag-input:focus-within {
    outline: 2px solid var(--accent);
    outline-offset: -1px;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    padding: 1px 2px 1px 8px;
    border-radius: 10px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 12px;
  }

  .chip button {
    padding: 0 5px;
    border: none;
    border-radius: 8px;
    color: inherit;
    line-height: 1.2;
  }

  input {
    flex: 1;
    min-width: 80px;
    padding: 2px;
    border: none;
    background: transparent;
  }

  input:focus {
    outline: none;
  }
</style>
