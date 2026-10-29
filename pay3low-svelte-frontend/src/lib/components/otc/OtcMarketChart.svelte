<script lang="ts">
  import { onMount } from "svelte";
  import { locale } from "$lib/i18n";
  import { getHistory, rate, type History, type Trade } from "$lib/otc/service";
  import { saleRate, type OfferHistoryPoint } from "$lib/otc/public-offers";
  import { chartRanges, type ChartRange } from "$lib/otc/market";
  import { dispose, init, type Chart, type DeepPartial, type KLineData, type Period, type Styles } from "klinecharts";

  export let trades: Trade[] = [];
  export let publicOffers: OfferHistoryPoint[] | null = null;

  type Mark = { id: string; time: number; value: number };

  let range: ChartRange = "1m";
  let history: History | null = null;
  let plot: HTMLDivElement;
  let chart: Chart | null = null;
  let signature = "";

  $: say = (en: string, ru: string) => $locale === "ru" ? ru : en;
  $: numbers = $locale === "ru" ? "ru-RU" : "en-US";

  // Participant prints stay as candles even when the server has no bucketed trades yet.
  $: marks = (publicOffers === null
    ? trades.map(trade => ({ id: trade.id, at: trade.created_at ?? "", price: rate(trade.terms), direction: trade.terms.direction }))
    : publicOffers.map(offer => ({ id: offer.id, at: offer.created_at, price: saleRate(offer.terms.ever_amount, offer.terms.usdt_amount), direction: offer.terms.direction })))
    .map(mark => ({ id: mark.id, time: Date.parse(mark.at), value: Number(mark.price) }))
    .filter(mark => Number.isFinite(mark.time) && Number.isFinite(mark.value) && mark.value > 0)
    .sort((a, b) => a.time - b.time);

  $: bars = buildBars(history, marks);
  $: latest = bars.length ? bars[bars.length - 1].close : null;
  $: change = bars.length > 1 && bars[0].open > 0 ? (bars[bars.length - 1].close - bars[0].open) / bars[0].open * 100 : null;
  $: precision = latest === null ? 8 : latest >= 100 ? 2 : latest >= 1 ? 3 : latest >= 0.01 ? 5 : 8;

  $: if (chart) synchronize(bars, precision, range, history?.bucket_seconds);

  function buildBars(source: History | null, prints: Mark[]): KLineData[] {
    const server = (source?.candles ?? []).flatMap(candle => candle.close === null ? [] : [{
      timestamp: candle.t * 1000,
      open: candle.open ?? candle.close,
      high: candle.high ?? candle.close,
      low: candle.low ?? candle.close,
      close: candle.close,
      volume: candle.volume ?? 0,
    }]);
    if (server.length) return server;
    // Without bucketed trades, every publication still becomes one candle so offers stay visible.
    const bars: KLineData[] = [];
    for (const mark of prints) {
      if (bars.at(-1)?.timestamp === mark.time) { bars[bars.length - 1].close = mark.value; continue; }
      const open = bars.length ? bars[bars.length - 1].close : mark.value;
      bars.push({ timestamp: mark.time, open, high: Math.max(open, mark.value), low: Math.min(open, mark.value), close: mark.value, volume: 0 });
    }
    return bars;
  }

  function periodFor(bucketSeconds: number | undefined): Period {
    const fallback = ({ "1h": 60, "24h": 900, "1w": 3600, "1m": 14400, "6m": 86400, "1y": 604800 } as Record<ChartRange, number>)[range];
    const bucket = bucketSeconds && bucketSeconds > 0 ? bucketSeconds : fallback;
    if (bucket < 60) return { type: "second", span: Math.max(1, bucket) };
    if (bucket < 3600) return { type: "minute", span: Math.max(1, Math.round(bucket / 60)) };
    if (bucket < 86400) return { type: "hour", span: Math.max(1, Math.round(bucket / 3600)) };
    return { type: "day", span: Math.max(1, Math.round(bucket / 86400)) };
  }

  function synchronize(data: KLineData[], pricePrecision: number, nextRange: ChartRange, bucketSeconds?: number) {
    if (!chart) return;
    const head = data[0], tail = data[data.length - 1];
    const next = `${data.length}|${head?.timestamp ?? 0}|${head?.open ?? 0}|${tail?.timestamp ?? 0}|${tail?.close ?? 0}|${pricePrecision}|${nextRange}`;
    if (next === signature) return;
    signature = next;
    chart.setSymbol({ ticker: "EVER/USDT", pricePrecision, volumePrecision: 2 });
    chart.setPeriod(periodFor(bucketSeconds));
    chart.resetData();
  }

  function choose(value: ChartRange) {
    if (range === value) return;
    range = value;
    history = null;
    signature = "";
    void load();
  }

  async function load() {
    try { history = await getHistory(range); } catch { history = null; }
  }

  function css(name: string, fallback: string) {
    if (typeof window === "undefined") return fallback;
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
  }

  function buildStyles(): DeepPartial<Styles> {
    const good = css("--color-good", "#247338");
    const danger = css("--color-danger", "#d43d35");
    const border = css("--color-border", "rgba(41, 67, 42, 0.16)");
    const text = css("--color-text", "#152016");
    const soft = css("--color-text-soft", "#5d6b5f");
    const faint = css("--color-text-faint", "#8a968c");
    const mono = 'ui-monospace, "SFMono-Regular", Consolas, monospace';
    return {
      grid: { show: true, horizontal: { show: true, color: border, size: 1, style: "dashed", dashedValue: [2, 4] }, vertical: { show: false } },
      candle: {
        type: "candle_solid",
        bar: {
          compareRule: "current_open",
          upColor: good, downColor: danger, noChangeColor: soft,
          upBorderColor: good, downBorderColor: danger, noChangeBorderColor: soft,
          upWickColor: good, downWickColor: danger, noChangeWickColor: soft,
        },
        priceMark: {
          show: true,
          high: { show: false, color: faint, textOffset: 8, textSize: 11, textFamily: mono, textWeight: "400" },
          low: { show: false, color: faint, textOffset: 8, textSize: 11, textFamily: mono, textWeight: "400" },
          last: {
            show: true, compareRule: "current_open", upColor: good, downColor: danger, noChangeColor: soft,
            line: { show: true, style: "dashed", size: 1, dashedValue: [3, 3], color: null },
            text: { show: true, style: "fill", color: "#ffffff", size: 11, family: mono, weight: "500", backgroundColor: null },
            extendTexts: [],
          },
        },
        tooltip: { showRule: "follow_cross", showType: "standard" },
      },
      indicator: {
        bars: [{ style: "fill", upColor: good, downColor: danger, noChangeColor: soft, borderSize: 0, borderStyle: "solid", borderDashedValue: [] }],
        tooltip: { showRule: "none" },
      },
      xAxis: { show: true, size: 24, axisLine: { show: false }, tickLine: { show: false, length: 0 }, tickText: { show: true, color: faint, size: 11, family: mono, marginStart: 6, marginEnd: 6 } },
      yAxis: { show: true, size: 78, axisLine: { show: false }, tickLine: { show: false, length: 0 }, tickText: { show: true, color: faint, size: 11, family: mono, marginStart: 6, marginEnd: 6 } },
      separator: { size: 8, color: border, fill: true, activeBackgroundColor: border },
      crosshair: {
        show: true,
        horizontal: { show: true, line: { show: true, color: faint, style: "dashed", dashedValue: [3, 3], size: 1 }, text: { show: true, style: "fill", color: "#ffffff", size: 11, family: mono, weight: "400", backgroundColor: text } },
        vertical: { show: true, line: { show: true, color: faint, style: "dashed", dashedValue: [3, 3], size: 1 }, text: { show: true, style: "fill", color: "#ffffff", size: 11, family: mono, weight: "400", backgroundColor: text } },
      },
    };
  }

  onMount(() => {
    chart = init(plot, { locale: "en-US", timezone: "UTC", styles: buildStyles() });
    if (chart) {
      chart.setSymbol({ ticker: "EVER/USDT", pricePrecision: precision, volumePrecision: 2 });
      chart.setPeriod(periodFor(history?.bucket_seconds));
      chart.createIndicator({ name: "VOL", calcParams: [] });
      chart.setDataLoader({ getBars: ({ callback }) => callback(bars) });
      chart.setOffsetRightDistance(12);
    }
    void load();
    const timer = setInterval(() => void load(), 60_000);
    const resize = new ResizeObserver(() => chart?.resize());
    resize.observe(plot);
    const theme = new MutationObserver(() => chart?.setStyles(buildStyles()));
    theme.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    return () => { clearInterval(timer); resize.disconnect(); theme.disconnect(); if (chart) dispose(plot); chart = null; };
  });
</script>

<div class="marketChart" data-points={marks.length}>
  <div class="chartHead">
    <div class="chartQuote">
      <span class="chartPair">EVER / USDT</span>
      <div class="chartValue">
        <strong data-testid="otc-chart-price">{latest === null ? "—" : `$${latest.toLocaleString(numbers, { maximumFractionDigits: Math.max(2, precision) })}`}</strong>
        {#if change !== null}<em class:negative={change < 0}>{change >= 0 ? "+" : "−"}{Math.abs(change).toFixed(2)}%</em>{/if}
      </div>
    </div>
    <div class="ranges" role="group" aria-label={say("Chart range", "Период графика")}>
      {#each chartRanges as item}<button type="button" class:active={range === item} aria-pressed={range === item} on:click={() => choose(item)}>{item}</button>{/each}
    </div>
  </div>

  <div class="chartPlot" bind:this={plot} role="img" aria-label={say("EVER price history in USDT per EVER", "История цены EVER в USDT за EVER")}>
    {#if !bars.length}
      <div class="empty">
        <strong>{publicOffers === null ? say("No quote history yet", "Истории котировок пока нет") : say("No offer history yet", "Истории предложений пока нет")}</strong>
        <p>{say("Prices appear here as the desk quotes and offers publish.", "Здесь появятся цены котировок деска и опубликованных предложений.")}</p>
      </div>
    {/if}
  </div>
</div>

<style>
  .marketChart { display: flex; width: 100%; min-width: 0; flex: 1; flex-direction: column; border: 1px solid var(--color-border-strong); border-radius: var(--otc-card-radius, 24px); background: var(--exchange-card-bg); overflow: hidden; }
  .chartHead { display: flex; flex-wrap: wrap; align-items: flex-end; justify-content: space-between; gap: 12px; padding: 20px var(--otc-panel-padding, 20px) 8px; }
  .chartQuote { display: grid; gap: 2px; min-width: 0; }
  .chartPair { color: var(--color-text-soft); font-family: var(--font-mono); font-size: 12px; letter-spacing: .04em; text-transform: uppercase; }
  .chartValue { display: flex; align-items: baseline; gap: 12px; min-width: 0; }
  .chartValue strong { font-family: var(--font-mono); font-size: clamp(34px, 4.4vw, 52px); font-weight: 500; line-height: 1.05; letter-spacing: -.04em; overflow-wrap: anywhere; }
  .chartValue em { color: var(--color-good); font-family: var(--font-mono); font-size: 15px; font-style: normal; }
  .chartValue em.negative { color: var(--color-danger); }
  .ranges { display: flex; flex: 0 1 auto; gap: 2px; padding: 3px; border: 1px solid var(--color-border); border-radius: 999px; background: var(--color-paper); }
  .ranges button { min-width: 40px; min-height: 34px; padding: 4px 8px; border-radius: 999px; color: var(--color-text-soft); font-family: var(--font-mono); font-size: 12px; line-height: 1.5; transition: background .18s ease, color .18s ease; }
  .ranges button:hover { color: var(--color-text); }
  .ranges button.active { background: var(--color-panel); color: var(--color-text); font-weight: 650; }
  .chartPlot { position: relative; width: 100%; height: clamp(320px, 34vw, 460px); padding: 0 4px 4px; }
  .empty { position: absolute; inset: 0; z-index: 2; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; padding: 24px; text-align: center; pointer-events: none; }
  .empty strong { background: var(--exchange-card-bg); font-size: 14px; font-weight: 650; padding: 5px 12px; }
  .empty p { max-width: 320px; background: var(--exchange-card-bg); color: var(--color-text-soft); font-size: 12px; line-height: 1.6; padding: 5px; }
  @media (max-width: 640px) {
    .chartHead { padding-inline: 12px; }
    .ranges { width: 100%; justify-content: space-between; }
    .ranges button { min-width: 0; flex: 1; }
    .chartPlot { height: 280px; }
  }
  @media (prefers-reduced-motion: reduce) { .ranges button { transition: none; } }
</style>
