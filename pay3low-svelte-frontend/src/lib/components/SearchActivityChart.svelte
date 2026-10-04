<script lang="ts">
  import { LayerCake, Svg } from "layercake";
  import { locale, t } from "$lib/i18n";
  import type { RouteSearchActivityHour } from "$lib/route-activity";
  import ActivityLine from "./ActivityLine.svelte";

  export let sourceCurrency: string;
  export let targetCurrency: string;
  export let hours: RouteSearchActivityHour[] = [];
  export let loading = false;
  export let error = false;

  type Point = { time: number; count: number };
  $: points = hours.map((hour) => ({ time: Date.parse(hour.started_at), count: hour.count })).filter((point) => Number.isFinite(point.time));
  $: lastIndex = Math.max(0, points.length - 1);
  $: total = points.reduce((sum, point) => sum + point.count, 0);
  $: yMaximum = Math.max(1, ...points.map((point) => point.count));
  $: hasSearches = points.some((point) => point.count > 0);

  function timeLabel(time: number): string {
    return new Intl.DateTimeFormat($locale === "ru" ? "ru-RU" : $locale === "hy" ? "hy-AM" : "en-US", { day: "numeric", month: "short", hour: "2-digit" }).format(time);
  }
</script>

<section class="activityCard" data-testid="search-activity" aria-label={t("Others searched this exchange", {}, $locale)} aria-busy={loading}>
  <div class="activityHeading"><div><small>{t("Search activity", {}, $locale)}</small><strong>{t("Others searched this exchange", {}, $locale)}</strong></div><span class="pair">{sourceCurrency} <span aria-hidden="true">→</span> {targetCurrency}</span></div>
  {#if error}
    <p class="state">{t("Search activity is temporarily unavailable.", {}, $locale)}</p>
  {:else if !points.length}
    <p class="state">{t("Loading search activity…", {}, $locale)}</p>
  {:else if points.length}
    <div class="activityStats"><strong>{total.toLocaleString($locale === "ru" ? "ru-RU" : "en-US")}</strong><span>{t("searches in the last 7 days", {}, $locale)}</span></div>
    <div class="mainPlot" aria-label={`${total} ${t("searches in the last 7 days", {}, $locale)}`}>
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
  .pair { flex: 0 0 auto; padding: 5px 8px; border-radius: 6px; background: var(--color-panel); color: var(--color-text-soft); font-size: 11px; font-weight: 750; }
  .activityStats { display: flex; align-items: baseline; gap: 5px; color: var(--color-text-faint); font-size: 11px; }
  .activityStats strong { color: var(--color-text); font-size: 18px; line-height: 1; }
  .mainPlot { position: relative; flex: 1 1 auto; min-height: 74px; }
  .emptyMessage { position: absolute; top: 45%; left: 50%; width: max-content; max-width: 95%; transform: translate(-50%, -50%); color: var(--color-text-faint); font-size: 11px; text-align: center; }
  .timeLabels { display: flex; justify-content: space-between; color: var(--color-text-faint); font-size: 10px; }
  .state { display: grid; min-height: 0; flex: 1; place-items: center; margin: 0; color: var(--color-text-faint); font-size: 12px; text-align: center; }
  :global(html[data-theme="dark"]) .activityCard { background: rgba(25,25,25,.96); }
</style>
