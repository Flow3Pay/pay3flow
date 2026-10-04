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
  let from = 120;
  let to = 167;
  let brushElement: HTMLDivElement;
  let drag: { mode: "move" | "start" | "end" | "new"; anchor: number; from: number; to: number } | null = null;

  $: points = hours.map((hour) => ({ time: Date.parse(hour.started_at), count: hour.count })).filter((point) => Number.isFinite(point.time));
  $: lastIndex = Math.max(0, points.length - 1);
  $: selectedFrom = Math.min(from, lastIndex);
  $: selectedTo = Math.min(to, lastIndex);
  $: focused = points.slice(selectedFrom, selectedTo + 1);
  $: total = focused.reduce((sum, point) => sum + point.count, 0);
  $: yMaximum = Math.max(1, ...points.map((point) => point.count));
  $: leftPercent = lastIndex ? selectedFrom / lastIndex * 100 : 0;
  $: widthPercent = lastIndex ? (selectedTo - selectedFrom) / lastIndex * 100 : 100;
  $: hasSearches = points.some((point) => point.count > 0);

  function timeLabel(time: number): string {
    return new Intl.DateTimeFormat($locale === "ru" ? "ru-RU" : $locale === "hy" ? "hy-AM" : "en-US", { day: "numeric", month: "short", hour: "2-digit" }).format(time);
  }

  function indexAt(clientX: number): number {
    const rect = brushElement.getBoundingClientRect();
    return Math.max(0, Math.min(lastIndex, Math.round((clientX - rect.left) / rect.width * lastIndex)));
  }

  function beginBrush(event: PointerEvent) {
    if (!points.length) return;
    const index = indexAt(event.clientX);
    const near = Math.max(2, Math.round(10 / brushElement.clientWidth * lastIndex));
    const mode = Math.abs(index - selectedFrom) <= near ? "start"
      : Math.abs(index - selectedTo) <= near ? "end"
      : index > selectedFrom && index < selectedTo ? "move" : "new";
    drag = { mode, anchor: index, from: selectedFrom, to: selectedTo };
    if (mode === "new") { from = index; to = Math.min(lastIndex, index + 1); }
    brushElement.setPointerCapture(event.pointerId);
    event.preventDefault();
  }

  function moveBrush(event: PointerEvent) {
    if (!drag) return;
    const index = indexAt(event.clientX);
    if (drag.mode === "start") from = Math.min(index, to - 1);
    else if (drag.mode === "end") to = Math.max(index, from + 1);
    else if (drag.mode === "move") {
      const width = drag.to - drag.from;
      from = Math.max(0, Math.min(lastIndex - width, drag.from + index - drag.anchor));
      to = from + width;
    } else {
      from = Math.min(index, drag.anchor);
      to = Math.max(index, drag.anchor + 1);
    }
  }

  function endBrush() { drag = null; }

  function adjustHandle(event: KeyboardEvent, edge: "start" | "end") {
    const step = event.shiftKey ? 24 : 1;
    const delta = event.key === "ArrowLeft" ? -step : event.key === "ArrowRight" ? step : 0;
    if (!delta) return;
    event.preventDefault();
    if (edge === "start") from = Math.max(0, Math.min(to - 1, from + delta));
    else to = Math.min(lastIndex, Math.max(from + 1, to + delta));
  }
</script>

<section class="activityCard" data-testid="search-activity" aria-label={t("Others searched this exchange", {}, $locale)} aria-busy={loading}>
  <div class="activityHeading"><div><small>{t("Search activity", {}, $locale)}</small><strong>{t("Others searched this exchange", {}, $locale)}</strong></div><span class="pair">{sourceCurrency} <span aria-hidden="true">→</span> {targetCurrency}</span></div>
  {#if error}
    <p class="state">{t("Search activity is temporarily unavailable.", {}, $locale)}</p>
  {:else if !points.length}
    <p class="state">{t("Loading search activity…", {}, $locale)}</p>
  {:else if points.length}
    <div class="activityStats"><strong>{total.toLocaleString($locale === "ru" ? "ru-RU" : "en-US")}</strong><span>{t("searches in selected period", {}, $locale)}</span></div>
    <div class="mainPlot" aria-label={`${total} ${t("searches in selected period", {}, $locale)}`}>
      <LayerCake data={focused} x="time" y="count" yDomain={[0, yMaximum]}>
        <Svg><ActivityLine /></Svg>
      </LayerCake>
      {#if !hasSearches}<span class="emptyMessage">{t("No searches by others recorded yet", {}, $locale)}</span>{/if}
    </div>
    <div class="timeLabels"><span>{timeLabel(focused[0]?.time ?? points[0].time)}</span><span>{timeLabel(focused[focused.length - 1]?.time ?? points[lastIndex].time)}</span></div>
    <div class="brush" bind:this={brushElement} role="group" aria-label={t("Select time range", {}, $locale)} on:pointerdown={beginBrush} on:pointermove={moveBrush} on:pointerup={endBrush} on:pointercancel={endBrush}>
      <LayerCake data={points} x="time" y="count" yDomain={[0, yMaximum]}>
        <Svg><ActivityLine overview /></Svg>
      </LayerCake>
      <div class="brushShade" style:left={`${leftPercent}%`} style:width={`${widthPercent}%`} aria-hidden="true"></div>
      <button type="button" class="handle" style:left={`${leftPercent}%`} aria-label={t("Adjust start of time range", {}, $locale)} on:keydown={(event) => adjustHandle(event, "start")}></button>
      <button type="button" class="handle" style:left={`${leftPercent + widthPercent}%`} aria-label={t("Adjust end of time range", {}, $locale)} on:keydown={(event) => adjustHandle(event, "end")}></button>
    </div>
    <div class="brushHint">{t("Drag to choose a period · last 7 days", {}, $locale)}</div>
  {/if}
</section>

<style>
  .activityCard { display: flex; min-width: 0; min-height: 0; flex-direction: column; gap: 6px; padding: 16px 18px 12px; border: 1px solid var(--color-border-strong); border-radius: var(--radius-card); background: rgba(255,255,255,.96); box-shadow: var(--shadow-card); color: var(--color-text); }
  .activityHeading { display: flex; justify-content: space-between; align-items: center; gap: 10px; }
  .activityHeading > div { display: flex; min-width: 0; flex-direction: column; gap: 1px; }
  .activityHeading small { color: var(--color-text-faint); font-size: 10px; font-weight: 700; letter-spacing: .09em; text-transform: uppercase; }
  .activityHeading strong { font-size: 14px; line-height: 1.2; }
  .pair { flex: 0 0 auto; padding: 5px 8px; border-radius: 6px; background: #eef5dd; color: #516922; font-size: 11px; font-weight: 750; }
  .activityStats { display: flex; align-items: baseline; gap: 5px; color: var(--color-text-faint); font-size: 11px; }
  .activityStats strong { color: var(--color-text); font-size: 18px; line-height: 1; }
  .mainPlot { position: relative; height: 74px; }
  .emptyMessage { position: absolute; top: 45%; left: 50%; width: max-content; max-width: 95%; transform: translate(-50%, -50%); color: var(--color-text-faint); font-size: 11px; text-align: center; }
  .timeLabels { display: flex; justify-content: space-between; color: var(--color-text-faint); font-size: 10px; }
  .brush { position: relative; height: 32px; overflow: hidden; border: 1px solid #d7dfca; border-radius: 5px; background: #f6f9f0; cursor: crosshair; touch-action: none; user-select: none; }
  .brushShade { position: absolute; z-index: 2; top: 0; bottom: 0; border-left: 1px solid #83aa2a; border-right: 1px solid #83aa2a; background: rgba(185,232,52,.2); pointer-events: none; }
  .handle { position: absolute; z-index: 3; top: 0; bottom: 0; width: 7px; padding: 0; transform: translateX(-50%); border: 0; border-radius: 3px; background: #729b17; cursor: ew-resize; }
  .handle:focus-visible { outline: 2px solid #20291b; outline-offset: -1px; }
  .brushHint { color: var(--color-text-faint); font-size: 10px; }
  .state { display: grid; min-height: 0; flex: 1; place-items: center; margin: 0; color: var(--color-text-faint); font-size: 12px; text-align: center; }
  :global(html[data-theme="dark"]) .activityCard { background: #1b201a; }
  :global(html[data-theme="dark"]) .pair { background: #344322; color: #d5f587; }
  :global(html[data-theme="dark"]) .brush { border-color: #435337; background: #21291d; }
</style>
