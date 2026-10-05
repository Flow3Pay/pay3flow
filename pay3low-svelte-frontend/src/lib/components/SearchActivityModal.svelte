<script lang="ts">
  import { onDestroy, onMount, tick } from "svelte";
  import { locale, t } from "$lib/i18n";
  import { lockPageScroll } from "$lib/page-scroll-lock";
  import { SEARCH_ACTIVITY_PERIOD_LABELS, SEARCH_ACTIVITY_PERIODS, type RouteSearchActivityHour, type SearchActivityPeriod } from "$lib/route-activity";
  import SearchActivityChart from "./SearchActivityChart.svelte";

  export let sourceCurrency: string;
  export let targetCurrency: string;
  export let hours: RouteSearchActivityHour[];
  export let period: SearchActivityPeriod;
  export let onPeriodChange: (period: SearchActivityPeriod) => void;
  export let loading = false;
  export let error = false;
  export let onClose: () => void;

  let dialog: HTMLDivElement;
  let periodDialog: HTMLDivElement;
  let periodPickerOpen = false;
  let unlock: (() => void) | undefined;
  let dragging = false;
  let dragStartY = 0;
  let dragDistance = 0;
  let dragDialog: HTMLDivElement;
  function keydown(event: KeyboardEvent) {
    if (event.key !== "Escape") return;
    if (periodPickerOpen) closePeriodPicker();
    else onClose();
  }
  function backdrop(event: MouseEvent) { if (event.target === event.currentTarget) onClose(); }
  async function openPeriodPicker() {
    periodPickerOpen = true;
    await tick();
    periodDialog?.focus();
  }
  function closePeriodPicker() {
    periodPickerOpen = false;
    void tick().then(() => dialog?.focus());
  }
  function choosePeriod(value: SearchActivityPeriod) {
    onPeriodChange(value);
    closePeriodPicker();
  }
  function startDrag(event: PointerEvent) {
    dragging = true;
    dragStartY = event.clientY;
    dragDistance = 0;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    dragDialog = (event.currentTarget as HTMLElement).parentElement as HTMLDivElement;
  }
  function moveDrag(event: PointerEvent) {
    if (!dragging) return;
    dragDistance = Math.max(0, event.clientY - dragStartY);
    dragDialog?.style.setProperty("--sheet-drag", `${dragDistance}px`);
  }
  function endDrag() {
    if (!dragging) return;
    dragging = false;
    if (dragDistance > 96 || dragDistance > dragDialog.clientHeight * 0.24) {
      if (periodPickerOpen) closePeriodPicker();
      else onClose();
    } else dragDialog?.style.removeProperty("--sheet-drag");
  }
  function cancelDrag() {
    dragging = false;
    dragDialog?.style.removeProperty("--sheet-drag");
  }
  onMount(async () => {
    unlock = lockPageScroll();
    window.addEventListener("keydown", keydown);
    await tick();
    dialog?.focus();
  });
  onDestroy(() => { unlock?.(); if (typeof window !== "undefined") window.removeEventListener("keydown", keydown); });
</script>

<div class="backdrop" role="presentation" on:mousedown={backdrop}>
  <div class:dragging class="dialog" bind:this={dialog} role="dialog" aria-modal="true" aria-label={t("Searches for this exchange", {}, $locale)} tabindex="-1">
    <button type="button" class="sheetHandle" aria-label={t("Close search activity", {}, $locale)} on:pointerdown={startDrag} on:pointermove={moveDrag} on:pointerup={endDrag} on:pointercancel={cancelDrag} on:keydown={(event) => { if (event.key === "Enter" || event.key === " ") onClose(); }}><span aria-hidden="true"></span></button>
    <button type="button" class="close" on:click={onClose} aria-label={t("Close search activity", {}, $locale)}><svg width="20" height="20" viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="m7 7 10 10m0-10L7 17" stroke="currentColor" stroke-width="2" stroke-linecap="round" /></svg></button>
    <SearchActivityChart {sourceCurrency} {targetCurrency} {hours} {period} {onPeriodChange} onOpenPeriodPicker={openPeriodPicker} {periodPickerOpen} {loading} {error} />
  </div>
  {#if periodPickerOpen}
    <div class="periodBackdrop" role="presentation" on:mousedown={closePeriodPicker}>
      <div class="periodDialog" bind:this={periodDialog} role="dialog" aria-modal="true" aria-label={t("Chart time range", {}, $locale)} tabindex="-1" on:mousedown|stopPropagation>
        <button type="button" class="periodHandle" aria-label={t("Chart time range", {}, $locale)} on:pointerdown={startDrag} on:pointermove={moveDrag} on:pointerup={endDrag} on:pointercancel={cancelDrag} on:keydown={(event) => { if (event.key === "Enter" || event.key === " ") closePeriodPicker(); }}><span aria-hidden="true"></span></button>
        <div class="periodHeading"><strong>{t("Chart time range", {}, $locale)}</strong></div>
        <div class="periodOptions">{#each SEARCH_ACTIVITY_PERIODS as value}<button type="button" aria-pressed={period === value} on:click={() => choosePeriod(value)}>{t(SEARCH_ACTIVITY_PERIOD_LABELS[value], {}, $locale)}</button>{/each}</div>
      </div>
    </div>
  {/if}
</div>

<style>
  .backdrop { position: fixed; z-index: 1000; inset: 0; display: grid; place-items: center; padding: 16px; background: rgba(13,15,12,.72); backdrop-filter: blur(12px); }
  .dialog { position: relative; width: min(100%, 560px); max-height: calc(100dvh - 32px); overflow: auto; outline: none; }
  .sheetHandle { display: none; }
  .close { position: absolute; z-index: 2; top: 12px; right: 12px; display: grid; width: 30px; height: 30px; place-items: center; border: 0; border-radius: 7px; background: #e9eee2; color: #283125; cursor: pointer; }
  .periodBackdrop { position: fixed; inset: 0; z-index: 3; display: grid; place-items: end center; background: rgba(8, 11, 8, .52); animation: fadeIn .2s ease-out; touch-action: none; }
  .periodDialog { width: 100%; max-height: calc(100dvh - 16px); overflow-y: auto; padding: 10px 16px calc(18px + env(safe-area-inset-bottom)); border: 1px solid var(--color-border); border-radius: 14px 14px 0 0; background: rgba(255, 255, 255, .98); box-shadow: var(--shadow-pop); touch-action: auto; transform: translateY(var(--sheet-drag, 0px)); transition: transform .24s ease; animation: sheetIn .24s cubic-bezier(.22, 1, .36, 1); }
  .periodHandle { display: flex; width: 100%; height: 30px; align-items: center; justify-content: center; touch-action: none; }
  .periodHandle span { width: 38px; height: 5px; border-radius: 999px; background: var(--color-border-strong); }
  .periodHeading strong { font-size: 13px; font-weight: 800; }
  .periodOptions { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 6px; margin-top: 14px; }
  .periodOptions button { min-height: 32px; padding: 0 9px; border: 1px solid var(--color-border); border-radius: 10px; background: var(--color-panel); color: var(--color-text-soft); font-size: 10px; font-weight: 800; }
  .periodOptions button[aria-pressed="true"] { border-color: var(--color-accent); background: var(--color-accent); color: #171717; box-shadow: 0 5px 13px rgba(185, 242, 39, .2); }
  :global(html[data-theme="dark"]) .periodDialog { border-color: var(--color-border-strong); background: rgba(25, 25, 25, .98); }
  :global(.dialog .activityHeading) { padding-right: 38px; }
  :global(.dialog .activityCard) { padding: 24px; gap: 14px; }
  :global(.dialog .mainPlot) { height: 180px; }
  @media (max-width: 640px) {
    .backdrop { align-items: end; padding: 0; background: rgba(8, 11, 8, .52); backdrop-filter: none; }
    .dialog { width: 100%; max-height: 86dvh; border-radius: 20px 20px 0 0; overflow: hidden; transform: translateY(var(--sheet-drag, 0px)); transition: transform .24s ease; animation: sheetIn .28s cubic-bezier(.22, 1, .36, 1); }
    .dialog.dragging { transition: none; }
    .sheetHandle { display: flex; width: 100%; height: 30px; align-items: center; justify-content: center; touch-action: none; }
    .sheetHandle span { width: 38px; height: 5px; border-radius: 999px; background: var(--color-border-strong); }
    .close { display: none; }
    :global(.dialog .activityCard) { border-radius: 20px 20px 0 0; padding: 16px 18px 24px; }
    :global(.dialog .mainPlot) { height: 155px; }
  }
  @keyframes sheetIn { from { transform: translateY(100%); } to { transform: translateY(0); } }
  @keyframes fadeIn { from { opacity: 0; } to { opacity: 1; } }
  @media (prefers-reduced-motion: reduce) { .dialog { animation: none; transition: none; } }
</style>
