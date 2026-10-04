<script lang="ts">
  import { LayerCake, Svg } from "layercake";
  import { locale, t } from "$lib/i18n";
  import { SEARCH_ACTIVITY_PERIODS, type RouteSearchActivityHour, type SearchActivityPeriod } from "$lib/route-activity";
  import ActivityLine from "./ActivityLine.svelte";

  export let sourceCurrency: string;
  export let targetCurrency: string;
  export let hours: RouteSearchActivityHour[] = [];
  export let period: SearchActivityPeriod = "1w";
  export let onPeriodChange: (period: SearchActivityPeriod) => void = () => {};
  export let loading = false;
  export let error = false;

  const periodLabels: Record<SearchActivityPeriod, string> = { "1h": "1 hour", "1d": "1 day", "1w": "1 week", "1m": "1 month", "3m": "3 months", "6m": "6 months", "1y": "1 year", all: "All time" };
  let periodMenuOpen = false;
  let periodMenu: HTMLDivElement;
  function closeOnOutsideClick(event: MouseEvent) {
    if (periodMenuOpen && periodMenu && !periodMenu.contains(event.target as Node)) periodMenuOpen = false;
  }
  function closeOnEscape(event: KeyboardEvent) {
    if (event.key === "Escape" && periodMenuOpen) { event.stopImmediatePropagation(); periodMenuOpen = false; }
  }
  function choosePeriod(value: SearchActivityPeriod) { onPeriodChange(value); periodMenuOpen = false; }

  type Point = { time: number; count: number };
  $: points = hours.map((hour) => ({ time: Date.parse(hour.started_at), count: hour.count })).filter((point) => Number.isFinite(point.time));
  $: lastIndex = Math.max(0, points.length - 1);
  $: total = points.reduce((sum, point) => sum + point.count, 0);
  $: yMaximum = Math.max(1, ...points.map((point) => point.count));
  $: hasSearches = points.some((point) => point.count > 0);
  $: periodLabel = t(periodLabels[period], {}, $locale);

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

<section class="activityCard" data-testid="search-activity" aria-label={t("Others searched this exchange", {}, $locale)} aria-busy={loading}>
  <div class="activityHeading"><div><small>{t("Search activity", {}, $locale)}</small><strong>{t("Others searched this exchange", {}, $locale)}</strong></div><div class="headingActions"><span class="pair">{sourceCurrency} <span aria-hidden="true">→</span> {targetCurrency}</span><div class="periodWrap" bind:this={periodMenu}><button type="button" class="periodButton" aria-label={t("Chart time range", {}, $locale)} aria-haspopup="menu" aria-expanded={periodMenuOpen} title={`${t("Chart time range", {}, $locale)}: ${periodLabel}`} on:click={() => periodMenuOpen = !periodMenuOpen}><img src="/icons/ui/chart-period.png" alt="" width="18" height="18" aria-hidden="true" /></button>{#if periodMenuOpen}<div class="periodMenu" role="menu" aria-label={t("Chart time range", {}, $locale)}>{#each SEARCH_ACTIVITY_PERIODS as value}<button type="button" role="menuitemradio" aria-checked={period === value} class:current={period === value} on:click={() => choosePeriod(value)}>{t(periodLabels[value], {}, $locale)}</button>{/each}</div>{/if}</div></div></div>
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
      {#if !hasSearches}<span class="emptyMessage">{t("No searches by others recorded yet", {}, $locale)}</span>{/if}
    </div>
    <div class="timeLabels"><span>{timeLabel(points[0].time)}</span><span>{timeLabel(points[lastIndex].time)}</span></div>
  {/if}
</section>

<style>
  .activityCard { display: flex; min-width: 0; min-height: 0; flex-direction: column; gap: 6px; padding: 16px 18px 12px; border: 1px solid var(--color-border-strong); border-radius: var(--radius-card); background: rgba(255,255,255,.96); box-shadow: var(--shadow-card); color: var(--color-text); }
  .activityHeading { display: flex; justify-content: space-between; align-items: center; gap: 10px; }
  .activityHeading > div { display: flex; min-width: 0; flex-direction: column; gap: 1px; }
  .activityHeading small { color: var(--color-text-faint); font-size: 10px; font-weight: 700; letter-spacing: .09em; text-transform: uppercase; }
  .activityHeading strong { font-size: 14px; line-height: 1.2; }
  .headingActions { display: flex; align-items: center; gap: 7px; flex: 0 0 auto; }
  .pair { flex: 0 0 auto; padding: 5px 8px; border-radius: 6px; background: var(--color-panel); color: var(--color-text-soft); font-size: 11px; font-weight: 750; }
  .periodWrap { position: relative; flex: 0 0 auto; }
  .periodButton { display: grid; width: 30px; height: 30px; place-items: center; padding: 0; border: 1px solid var(--color-border-strong); border-radius: 7px; background: var(--color-panel); cursor: pointer; }
  .periodButton:hover, .periodButton[aria-expanded="true"] { border-color: var(--color-text-soft); }
  .periodButton img { width: 18px; height: 18px; object-fit: contain; }
  :global(html[data-theme="dark"]) .periodButton img { filter: invert(1); }
  .periodMenu { position: absolute; z-index: 10; top: calc(100% + 7px); right: 0; display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 4px; width: 216px; padding: 8px; border: 1px solid var(--color-border-strong); border-radius: 10px; background: var(--color-panel); box-shadow: var(--shadow-card); }
  .periodMenu button { min-height: 34px; padding: 5px 8px; border: 0; border-radius: 6px; background: transparent; color: var(--color-text); font: inherit; font-size: 11px; font-weight: 650; text-align: left; cursor: pointer; }
  .periodMenu button:hover, .periodMenu button.current { background: var(--color-border-strong); }
  @media (max-width: 420px) { .activityHeading { align-items: flex-start; } .headingActions { gap: 4px; } .pair { padding-inline: 5px; font-size: 10px; } .activityHeading strong { font-size: 12px; } }
  .activityStats { display: flex; align-items: baseline; gap: 5px; color: var(--color-text-faint); font-size: 11px; }
  .activityStats strong { color: var(--color-text); font-size: 18px; line-height: 1; }
  .mainPlot { position: relative; flex: 1 1 auto; min-height: 74px; }
  .emptyMessage { position: absolute; top: 45%; left: 50%; width: max-content; max-width: 95%; transform: translate(-50%, -50%); color: var(--color-text-faint); font-size: 11px; text-align: center; }
  .timeLabels { display: flex; justify-content: space-between; color: var(--color-text-faint); font-size: 10px; }
  .state { display: grid; min-height: 0; flex: 1; place-items: center; margin: 0; color: var(--color-text-faint); font-size: 12px; text-align: center; }
  :global(html[data-theme="dark"]) .activityCard { background: rgba(25,25,25,.96); }
</style>
