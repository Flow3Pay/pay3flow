<script lang="ts">
  import { onDestroy, onMount, tick } from "svelte";
  import { locale, t } from "$lib/i18n";
  import { lockPageScroll } from "$lib/page-scroll-lock";
  import type { RouteSearchActivityHour } from "$lib/route-activity";
  import SearchActivityChart from "./SearchActivityChart.svelte";

  export let sourceCurrency: string;
  export let targetCurrency: string;
  export let hours: RouteSearchActivityHour[];
  export let loading = false;
  export let error = false;
  export let onClose: () => void;

  let dialog: HTMLDivElement;
  let unlock: (() => void) | undefined;
  function keydown(event: KeyboardEvent) { if (event.key === "Escape") onClose(); }
  function backdrop(event: MouseEvent) { if (event.target === event.currentTarget) onClose(); }
  onMount(async () => {
    unlock = lockPageScroll();
    window.addEventListener("keydown", keydown);
    await tick();
    dialog?.focus();
  });
  onDestroy(() => { unlock?.(); if (typeof window !== "undefined") window.removeEventListener("keydown", keydown); });
</script>

<div class="backdrop" role="presentation" on:mousedown={backdrop}>
  <div class="dialog" bind:this={dialog} role="dialog" aria-modal="true" aria-label={t("Others searched this exchange", {}, $locale)} tabindex="-1">
    <button type="button" class="close" on:click={onClose} aria-label={t("Close search activity", {}, $locale)}><svg width="20" height="20" viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="m7 7 10 10m0-10L7 17" stroke="currentColor" stroke-width="2" stroke-linecap="round" /></svg></button>
    <SearchActivityChart {sourceCurrency} {targetCurrency} {hours} {loading} {error} />
  </div>
</div>

<style>
  .backdrop { position: fixed; z-index: 1000; inset: 0; display: grid; place-items: center; padding: 16px; background: rgba(13,15,12,.72); backdrop-filter: blur(12px); }
  .dialog { position: relative; width: min(100%, 560px); max-height: calc(100dvh - 32px); overflow: auto; outline: none; }
  .close { position: absolute; z-index: 2; top: 12px; right: 12px; display: grid; width: 30px; height: 30px; place-items: center; border: 0; border-radius: 7px; background: #e9eee2; color: #283125; cursor: pointer; }
  :global(.dialog .activityHeading) { padding-right: 38px; }
  :global(.dialog .activityCard) { padding: 24px; gap: 14px; }
  :global(.dialog .mainPlot) { height: 180px; }
  @media (max-width: 640px) { .backdrop { align-items: end; padding: 0; } .dialog { width: 100%; max-height: 86dvh; border-radius: 20px 20px 0 0; overflow: hidden; } :global(.dialog .activityCard) { border-radius: 20px 20px 0 0; padding: 28px 18px 24px; } :global(.dialog .mainPlot) { height: 155px; } }
</style>
