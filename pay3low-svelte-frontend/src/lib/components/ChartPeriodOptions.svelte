<script lang="ts">
  import type { Snippet } from "svelte";
  import { locale, t } from "$lib/i18n";
  import { SEARCH_ACTIVITY_PERIOD_LABELS, SEARCH_ACTIVITY_PERIODS, type SearchActivityPeriod } from "$lib/route-activity";

  export let period: SearchActivityPeriod;
  export let onSelect: (period: SearchActivityPeriod) => void;
  export let onClose: () => void;
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
  <div class="periodModalHeader">
    {@render sheetHandle?.()}
    <button type="button" class="periodClose" on:click={onClose} aria-label={t("Close chart time range", {}, $locale)}><svg width="20" height="20" viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="m7 7 10 10m0-10L7 17" stroke="currentColor" stroke-width="2" stroke-linecap="round" /></svg></button>
  </div>
  <div class="periodHeading"><strong>{t("Chart time range", {}, $locale)}</strong></div>
  <div class="periodOptions" aria-label={t("Chart time range", {}, $locale)}>
    {#each SEARCH_ACTIVITY_PERIODS as value}
      <button type="button" class:current={period === value} aria-pressed={period === value} on:click={() => onSelect(value)}>{t(SEARCH_ACTIVITY_PERIOD_LABELS[value], {}, $locale)}</button>
    {/each}
  </div>
</div>

<style>
  .periodPickerContent { position: relative; }
  .periodModalHeader { display: flex; justify-content: flex-end; }
  .periodClose { display: grid; width: 32px; height: 32px; flex: 0 0 auto; place-items: center; border-radius: 9px; color: var(--color-text-soft); }
  .periodClose:hover { background: var(--color-panel); }
  .periodHeading { padding-right: 36px; margin-top: -26px; }
  .periodHeading strong { font-size: 13px; font-weight: 800; }
  .periodOptions { display: grid; grid-template-rows: repeat(4, minmax(32px, auto)); grid-auto-flow: column; grid-auto-columns: minmax(130px, 1fr); gap: 6px; margin-top: 14px; }
  .periodOptions button { min-height: 32px; padding: 0 9px; border: var(--border-highlight-width) solid var(--color-border); border-radius: 10px; background: var(--color-panel); color: var(--color-text-soft); font-size: 12px; font-weight: 800; transition: border-color .15s ease, background .15s ease, color .15s ease; }
  .periodOptions button:hover { border-color: var(--color-accent); }
  .periodOptions button.current { border-color: var(--color-accent); background: var(--color-accent); color: #171717; }
  @media (max-width: 640px) {
    .periodModalHeader { min-height: 30px; align-items: center; justify-content: space-between; }
    .periodClose { width: 44px; height: 44px; }
    .periodHeading { margin-top: 0; padding-right: 0; }
    .periodOptions { grid-auto-flow: row; grid-template-rows: none; grid-auto-columns: auto; grid-template-columns: repeat(2, minmax(0, 1fr)); }
    .periodOptions button { min-height: 44px; }
  }
</style>
