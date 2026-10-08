<script lang="ts">
  import { tick, type Snippet } from "svelte";
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
  function closeOnOutsideClick(event: MouseEvent) {
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
  $: points = hours.map((hour) => ({ time: Date.parse(hour.started_at), count: hour.count })).filter((point) => Number.isFinite(point.time));
  $: lastIndex = Math.max(0, points.length - 1);
  $: total = points.reduce((sum, point) => sum + point.count, 0);
  $: yMaximum = Math.max(1, ...points.map((point) => point.count));
  $: hasSearches = points.some((point) => point.count > 0);
  $: periodLabel = t(SEARCH_ACTIVITY_PERIOD_LABELS[period], {}, $locale);

  function timeLabel(time: number): string {
    const options: Intl.DateTimeFormatOptions = period === "1h"
      ? { hour: "2-digit", minute: "2-digit" }
      : period === "1d" || period === "1w"
        ? { day: "numeric", month: "short", hour: "2-digit" }
        : { day: "numeric", month: "short", year: "numeric" };
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
      <div class="periodWrap" bind:this={periodMenu}><button type="button" class="periodButton" bind:this={periodButton} aria-label={t("Chart time range", {}, $locale)} aria-haspopup="dialog" aria-expanded={periodMenuOpen || periodPickerOpen} title={`${t("Chart time range", {}, $locale)}: ${periodLabel}`} on:click={togglePeriodPicker}><img src="/icons/ui/chart-period.png" alt="" width="18" height="18" aria-hidden="true" /></button>{#if periodMenuOpen}<div class="periodMenu" role="dialog" aria-label={t("Chart time range", {}, $locale)} tabindex="-1"><ChartPeriodOptions {period} onSelect={choosePeriod} onClose={closePeriodMenu} /></div>{/if}</div>
    </div>
  </div>
  {#if error}
    <p class="state">{t("Search activity is temporarily unavailable.", {}, $locale)}</p>
  {:else if !points.length}
    <p class="state">{t("Loading search activity…", {}, $locale)}</p>
  {:else if points.length}
    <div class="activityStats"><strong>{total.toLocaleString($locale === "ru" ? "ru-RU" : "en-US")}</strong><span>{t("searches in period", {}, $locale)} · {periodLabel}</span></div>
    <div class="mainPlot" aria-label={`${total} ${t("searches in period", {}, $locale)} · ${periodLabel}`}>
      <LayerCake data={points} x="time" y="count" yDomain={[0, yMaximum]}>
        <Svg><ActivityLine /></Svg>
      </LayerCake>
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
  .mainPlot { position: relative; height: 74px; flex: 1 1 74px; min-height: 74px; }
  .emptyMessage { position: absolute; top: 45%; left: 50%; width: max-content; max-width: 95%; transform: translate(-50%, -50%); color: var(--color-text-faint); font-size: 12px; text-align: center; }
  .timeLabels { display: flex; justify-content: space-between; color: var(--color-text-faint); font-size: 12px; }
  .state { display: grid; min-height: 0; flex: 1; place-items: center; margin: 0; color: var(--color-text-faint); font-size: 12px; text-align: center; }
  :global(html[data-theme="dark"]) .activityCard { background: rgba(25,25,25,.96); }
</style>
