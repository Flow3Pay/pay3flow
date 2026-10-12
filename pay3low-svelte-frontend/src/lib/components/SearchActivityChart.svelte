<script lang="ts">
  import { tick, type Snippet } from "svelte";
  import { fly } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { LayerCake, Svg } from "layercake";
  import { locale, t } from "$lib/i18n";
  import { fiatFlagUrl } from "$lib/currency-flags";
  import { assetIcon } from "$lib/icons";
  import { SEARCH_ACTIVITY_PERIOD_LABELS, type RouteSearchActivityHour, type SearchActivityPeriod } from "$lib/route-activity";
  import ActivityLine from "./ActivityLine.svelte";
  import ChartPeriodOptions from "./ChartPeriodOptions.svelte";

  export let sourceCurrency: string;
  export let targetCurrency: string;
  export let hours: RouteSearchActivityHour[] = [];
  export let period: SearchActivityPeriod = "1w";
  export let onPeriodChange: (period: SearchActivityPeriod) => void = () => {};
  export let onOpenPeriodPicker: (() => void) | undefined = undefined;
  export let periodPickerOpen = false;
  export let loading = false;
  export let error = false;
  export let sheetHandle: Snippet | undefined = undefined;

  let periodMenuOpen = false;
  let periodMenu: HTMLDivElement;
  let periodButton: HTMLButtonElement;
  let plot: HTMLDivElement;
  let activeIndex: number | null = null;
  let hovered: { time: number; count: number; x: number; width: number } | null = null;
  function periodMenuTransition(node: Element) {
    return fly(node, { y: -6, duration: window.matchMedia("(prefers-reduced-motion: reduce)").matches ? 0 : 180, easing: cubicOut });
  }
  function closeOnOutsideClick(event: MouseEvent) {
    if (plot && !plot.contains(event.target as Node)) activeIndex = null;
    if (periodMenuOpen && periodMenu && !periodMenu.contains(event.target as Node)) closePeriodMenu();
  }
  function closeOnEscape(event: KeyboardEvent) {
    if (event.key === "Escape" && periodMenuOpen) { event.stopImmediatePropagation(); closePeriodMenu(); }
  }
  function closePeriodMenu() { periodMenuOpen = false; periodButton?.focus(); }
  function choosePeriod(value: SearchActivityPeriod) { onPeriodChange(value); closePeriodMenu(); }
  async function togglePeriodPicker() {
    if (onOpenPeriodPicker && window.matchMedia("(max-width: 640px)").matches) {
      periodMenuOpen = false;
      onOpenPeriodPicker();
    }
    else if (periodMenuOpen) closePeriodMenu();
    else {
      periodMenuOpen = true;
      await tick();
      periodMenu?.querySelector<HTMLButtonElement>('[aria-pressed="true"]')?.focus();
    }
  }

  type Point = { time: number; count: number };
  $: points = hours.map((hour) => ({ time: Date.parse(hour.started_at), count: hour.count })).filter((point) => Number.isFinite(point.time)).sort((a, b) => a.time - b.time);
  $: lastIndex = Math.max(0, points.length - 1);
  $: total = points.reduce((sum, point) => sum + point.count, 0);
  $: yMaximum = Math.max(4, Math.ceil(Math.max(0, ...points.map((point) => point.count)) / 4) * 4);
  $: hasSearches = points.some((point) => point.count > 0);
  $: periodLabel = t(SEARCH_ACTIVITY_PERIOD_LABELS[period], {}, $locale);

  function timeLabel(time: number): string {
    const options: Intl.DateTimeFormatOptions = period === "1h"
      ? { hour: "2-digit", minute: "2-digit", timeZone: "UTC" }
      : period === "1d" || period === "1w"
        ? { day: "numeric", month: "short", hour: "2-digit", timeZone: "UTC" }
        : { day: "numeric", month: "short", year: "numeric", timeZone: "UTC" };
    return new Intl.DateTimeFormat($locale === "ru" ? "ru-RU" : $locale === "hy" ? "hy-AM" : "en-US", options).format(time);
  }
</script>

<svelte:window on:click={closeOnOutsideClick} on:keydown={closeOnEscape} />

<section class="activityCard" data-testid="search-activity" aria-label={t("Searches for this exchange", {}, $locale)} aria-busy={loading}>
  {@render sheetHandle?.()}
  <div class="activityHeading">
    <div class="headingCopy"><strong>{t("Searches for this exchange", {}, $locale)}</strong></div>
    <div class="headingActions">
      <span class="pair">
        <span class="pairCurrency">
          <img src={fiatFlagUrl(sourceCurrency) ?? assetIcon(sourceCurrency)} alt="" width="16" height="16" aria-hidden="true" />
          {sourceCurrency}
        </span>
        <span aria-hidden="true">→</span>
        <span class="pairCurrency">
          <img src={fiatFlagUrl(targetCurrency) ?? assetIcon(targetCurrency)} alt="" width="16" height="16" aria-hidden="true" />
          {targetCurrency}
        </span>
      </span>
      <div class="periodWrap" bind:this={periodMenu}><button type="button" class="periodButton" bind:this={periodButton} aria-label={t("Chart time range", {}, $locale)} aria-haspopup="dialog" aria-expanded={periodMenuOpen || periodPickerOpen} title={`${t("Chart time range", {}, $locale)}: ${periodLabel}`} on:click={togglePeriodPicker}><img src="/icons/ui/chart-period.png" alt="" width="18" height="18" aria-hidden="true" /></button>{#if periodMenuOpen}<div class="periodMenu" role="dialog" aria-label={t("Chart time range", {}, $locale)} tabindex="-1" transition:periodMenuTransition><ChartPeriodOptions {period} onSelect={choosePeriod} /></div>{/if}</div>
    </div>
  </div>
  {#if error}
    <p class="state">{t("Search activity is temporarily unavailable.", {}, $locale)}</p>
  {:else if !points.length}
    <p class="state">{t("Loading search activity…", {}, $locale)}</p>
  {:else if points.length}
    <div class="activityStats"><strong>{total.toLocaleString($locale === "ru" ? "ru-RU" : "en-US")}</strong><span>{t("searches in period", {}, $locale)} · {periodLabel}</span></div>
    <div class="mainPlot" bind:this={plot} aria-label={`${total} ${t("searches in period", {}, $locale)} · ${periodLabel}`}>
      <LayerCake data={points} x="time" y="count" yDomain={[0, yMaximum]} padding={{ top: 8, right: 36, bottom: 6, left: 4 }}>
        <Svg label={`${sourceCurrency} → ${targetCurrency} · ${total} ${t("searches in period", {}, $locale)} · ${periodLabel}`}><ActivityLine maximum={yMaximum} {timeLabel} label={t("Searches for this exchange", {}, $locale)} bind:activeIndex onHover={(point) => { hovered = point; }} countLabel={(count) => `${count} ${t("searches in period", {}, $locale)}`} /></Svg>
      </LayerCake>
      {#if hovered}
        <div class="activityTooltip" role="tooltip" style:left={`${Math.max(8, Math.min(hovered.x + 16, hovered.width - 160))}px`}>
          <small>{timeLabel(hovered.time)}</small>
          <span><strong>{hovered.count.toLocaleString($locale)}</strong> {t("searches in period", {}, $locale)}</span>
        </div>
      {/if}
      {#if !hasSearches}<span class="emptyMessage">{t("No searches recorded yet", {}, $locale)}</span>{/if}
    </div>
    <div class="timeLabels"><span>{timeLabel(points[0].time)}</span><span>{timeLabel(points[lastIndex].time)}</span></div>
  {/if}
</section>

<style>
  .activityCard { display: flex; min-width: 0; min-height: 0; flex-direction: column; gap: 6px; padding: 16px 18px 12px; border: 1px solid var(--color-border-strong); border-radius: var(--radius-card); background: rgba(255,255,255,.96); box-shadow: none; color: var(--color-text); }
  .activityHeading { display: flex; justify-content: space-between; align-items: center; gap: 10px; }
  .headingCopy { display: flex; min-width: 0; flex-direction: column; gap: 1px; }
  .activityHeading strong { font-size: 14px; line-height: 1.2; }
  .headingActions { display: flex; align-items: center; gap: 7px; flex: 0 0 auto; }
  .pair { display: inline-flex; flex: 0 0 auto; align-items: center; gap: 5px; padding: 5px 8px; border-radius: 6px; background: var(--color-panel); color: var(--color-text-soft); font-size: 12px; font-weight: 750; }
  .pairCurrency { display: inline-flex; align-items: center; gap: 4px; white-space: nowrap; }
  .pairCurrency img { display: block; width: 16px; height: 16px; border-radius: 50%; object-fit: cover; }
  .periodWrap { position: relative; flex: 0 0 auto; }
  .periodButton { display: grid; width: 32px; height: 32px; place-items: center; padding: 0; border: 0; border-radius: 9px; background: transparent; cursor: pointer; }
  .periodButton:focus-visible { outline: var(--focus-ring-width) solid var(--color-focus); outline-offset: 2px; }
  @media (max-width: 980px), (pointer: coarse) { .periodButton { width: 44px; height: 44px; } .headingActions { margin-right: -6px; } }
  .periodButton img { width: 18px; height: 18px; object-fit: contain; filter: brightness(0); }
  :global(html[data-theme="dark"]) .periodButton img { filter: none; }
  .periodMenu { position: absolute; z-index: 80; top: calc(100% + 9px); right: 0; width: 310px; max-width: calc(100vw - 32px); max-height: min(680px, calc(100dvh - 32px)); overflow-y: auto; padding: 17px; border: 1px solid var(--color-border); border-radius: 20px; background: rgba(255,255,255,.97); box-shadow: none; }
  :global(html[data-theme="dark"]) .periodMenu { border-color: var(--color-border-strong); background: #191919; }
  @media (max-width: 420px) { .activityHeading { align-items: flex-start; } .headingActions { gap: 4px; } .pair { gap: 3px; padding-inline: 5px; font-size: 12px; } .pairCurrency { gap: 3px; } .pairCurrency img { width: 14px; height: 14px; } .activityHeading strong { font-size: 12px; } }
  .activityStats { display: flex; align-items: baseline; gap: 5px; color: var(--color-text-faint); font-size: 12px; }
  .activityStats strong { color: var(--color-text); font-size: 18px; line-height: 1; }
  .mainPlot { position: relative; height: 150px; flex: 1 1 150px; min-height: 120px; }
  .emptyMessage { position: absolute; top: 45%; left: 50%; width: max-content; max-width: 95%; transform: translate(-50%, -50%); padding: 5px 12px; background: var(--exchange-card-bg); color: var(--color-text-soft); font-size: 12px; text-align: center; pointer-events: none; }
  .activityTooltip { position: absolute; z-index: 1; top: 8px; width: 160px; max-width: calc(100% - 16px); padding: 9px 10px; border: 1px solid var(--color-border-strong); border-radius: 7px; background: var(--color-paper); color: var(--color-text); box-shadow: 0 4px 16px #0001; pointer-events: none; font-size: 11px; }
  .activityTooltip small { display: block; margin-bottom: 4px; color: var(--color-text-soft); font-size: 10px; }
  .activityTooltip strong { color: var(--color-text); font-family: var(--font-mono); }
  .timeLabels { display: flex; justify-content: space-between; gap: 12px; padding: 0 36px 0 4px; color: var(--color-text-faint); font-size: 11px; }
  .state { display: grid; min-height: 0; flex: 1; place-items: center; margin: 0; color: var(--color-text-faint); font-size: 12px; text-align: center; }
  :global(html[data-theme="dark"]) .activityCard { background: rgba(25,25,25,.96); }
</style>
