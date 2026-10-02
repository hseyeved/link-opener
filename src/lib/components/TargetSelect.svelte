<script lang="ts">
  import { onMount } from "svelte";
  import { app } from "$lib/state.svelte";
  import { describeTarget, optionKey, optionLabel, targetOptions, toTarget } from "$lib/targets";
  import type { LaunchTarget } from "$lib/types";

  interface Props {
    /** `null` = no default of its own. */
    value: LaunchTarget | null;
    /** Label for the `null` choice, e.g. "Ask every time" or what is inherited. */
    emptyLabel: string;
  }

  let { value = $bindable(), emptyLabel }: Props = $props();

  const MISSING = "missing";
  const options = $derived(targetOptions(app.browsers ?? []));
  const exact = $derived(
    value
      ? options.find((o) => o.browser.id === value!.browserId && (o.profile?.id ?? null) === value!.profileId)
      : undefined,
  );
  const key = $derived(!value ? "" : exact ? optionKey(exact) : MISSING);
  const supportsPrivate = $derived(exact?.browser.supportsPrivate ?? false);

  onMount(() => {
    if (!app.browsers) app.loadBrowsers().catch(() => {});
  });

  function onchange(e: Event) {
    const k = (e.currentTarget as HTMLSelectElement).value;
    if (!k) {
      value = null;
      return;
    }
    const option = options.find((o) => optionKey(o) === k);
    if (option) value = toTarget(option, (value?.private ?? false) && option.browser.supportsPrivate);
  }
</script>

<div class="target-select">
  <select value={key} {onchange} aria-label="Default browser">
    <option value="">{emptyLabel}</option>
    {#each options as option (optionKey(option))}
      <option value={optionKey(option)}>{optionLabel(option)}</option>
    {/each}
    {#if key === MISSING && value}
      <option value={MISSING} disabled>{describeTarget(value, app.browsers)}</option>
    {/if}
  </select>
  <label class="private" class:disabled={!supportsPrivate}>
    <input
      type="checkbox"
      checked={value?.private ?? false}
      disabled={!value || !supportsPrivate}
      onchange={(e) => {
        if (value) value = { ...value, private: e.currentTarget.checked };
      }}
    />
    Private
  </label>
</div>

<style>
  .target-select {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  select {
    flex: 1;
    min-width: 0;
  }

  .private {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 13px;
    white-space: nowrap;
  }

  .private.disabled {
    color: var(--muted);
  }

  .private input {
    margin: 0;
  }
</style>
