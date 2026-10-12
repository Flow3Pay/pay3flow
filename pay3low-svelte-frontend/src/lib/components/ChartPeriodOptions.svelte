<script lang="ts">
  import type { Snippet } from "svelte";
  import { locale, t } from "$lib/i18n";
  import { SEARCH_ACTIVITY_PERIOD_LABELS, SEARCH_ACTIVITY_PERIODS, type SearchActivityPeriod } from "$lib/route-activity";

  export let period: SearchActivityPeriod;
  export let onSelect: (period: SearchActivityPeriod) => void;
  export let sheetHandle: Snippet | undefined = undefined;
  let content: HTMLDivElement;

  function trapFocus(event: KeyboardEvent) {
    if (event.key !== "Tab" || !content?.contains(document.activeElement)) return;
    const buttons = Array.from(content.querySelectorAll<HTMLButtonElement>("button"));
    const first = buttons[0], last = buttons.at(-1);
    if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
    else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
  }
</script>

<svelte:window on:keydown={trapFocus} />

<div class="periodPickerContent" bind:this={content}>
  {#if sheetHandle}
    <div class="periodModalHeader">{@render sheetHandle()}</div>
  {/if}
  <div class="periodHeading"><strong>{t("Chart time range", {}, $locale)}</strong></div>
  <div class="periodOptions" aria-label={t("Chart time range", {}, $locale)}>
    {#each SEARCH_ACTIVITY_PERIODS as value}
      <button type="button" class:current={period === value} aria-pressed={period === value} on:click={() => onSelect(value)}>{t(SEARCH_ACTIVITY_PERIOD_LABELS[value], {}, $locale)}</button>
    {/each}
  </div>
</div>

<style>
  .periodPickerContent { position: relative; }
  .periodModalHeader { display: flex; min-height: 30px; align-items: center; }
  .periodHeading strong { font-size: 13px; font-weight: 800; }
  .periodOptions { display: grid; grid-template-rows: repeat(4, minmax(32px, auto)); grid-auto-flow: column; grid-auto-columns: minmax(130px, 1fr); gap: 6px; margin-top: 14px; }
  .periodOptions button { min-height: 32px; padding: 0 9px; border: var(--border-highlight-width) solid var(--color-border); border-radius: 10px; background: var(--color-panel); color: var(--color-text-soft); font-size: 12px; font-weight: 800; transition: border-color .15s ease, background .15s ease, color .15s ease; }
  .periodOptions button:hover { border-color: var(--color-accent); }
  .periodOptions button.current { border-color: var(--color-accent); background: var(--color-accent); color: #171717; }
  @media (max-width: 640px) {
    .periodOptions { grid-auto-flow: row; grid-template-rows: none; grid-auto-columns: auto; grid-template-columns: repeat(2, minmax(0, 1fr)); }
    .periodOptions button { min-height: 44px; }
  }
</style>
