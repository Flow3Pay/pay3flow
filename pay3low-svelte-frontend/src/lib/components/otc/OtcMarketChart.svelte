<script lang="ts">
  import { afterUpdate, onMount } from "svelte";
  import { locale } from "$lib/i18n";
  import { getHistory, rate, type History, type Trade } from "$lib/otc/service";
  import { saleRate, type OfferHistoryPoint } from "$lib/otc/public-offers";
  import { candleIntervalLabel, chartRanges, chartWindow, chartWindows, markCandles, type ChartRange, type ChartWindow } from "$lib/otc/market";
  import { dispose, init, registerLocale, type Chart, type DeepPartial, type KLineData, type Period, type Styles } from "klinecharts";

  // klinecharts ships `minute: ""`, so a 15-minute series renders as a bare "15".
  registerLocale("en-US", {
    time: "Time: ", open: "Open: ", high: "High: ", low: "Low: ", close: "Close: ",
    volume: "Volume: ", turnover: "Turnover: ", change: "Change: ",
    second: "s", minute: "m", hour: "H", day: "D", week: "W", month: "M", year: "Y",
  });

  export let trades: Trade[] = [];
  export let publicOffers: OfferHistoryPoint[] | null = null;

  type Mark = { id: string; time: number; value: number };

  // v2: the buttons are candle timeframes now, so an old month/week choice must not be reused.
  const rangeStorageKey = "pay3flow.otc.chart-range.v2";
  function savedRange(): ChartRange {
    try {
      const value = localStorage.getItem(rangeStorageKey);
      return chartRanges.includes(value as ChartRange) ? value as ChartRange : "1h";
    } catch { return "1h"; }
  }

  let range: ChartRange = "1h";
  let history: History | null = null;
  let plot: HTMLDivElement;
  let ranges: HTMLDivElement;
  let rangeButtons: HTMLButtonElement[] = [];
  let thumb = { offset: 0, width: 0, ready: false };
  let mounted = false;
  let swapping = false;
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

  $: bounds = chartBounds(history, range, marks.at(-1)?.time);
  $: layout = buildLayout(history, marks, bounds);
  $: bars = layout.bars;
  // The hero price keeps the latest print even when the selected range has no candles.
  $: latest = bars.length ? bars[bars.length - 1].close : marks.at(-1)?.value ?? null;
  $: change = bars.length > 1 && bars[0].open > 0 ? (bars[bars.length - 1].close - bars[0].open) / bars[0].open * 100 : null;
  $: precision = latest === null ? 8 : latest >= 100 ? 2 : latest >= 1 ? 3 : latest >= 0.01 ? 5 : 8;

  $: if (chart) synchronize(bars, precision, layout.window);

  /**
   * The chart always keeps the selected range's grid. The server's window start
   * is used when available (it is aligned to a bucket edge), otherwise the
   * local grid anchors on `now` so a just-published offer stays in view.
   */
  function chartBounds(source: History | null, selectedRange: ChartRange, latestPrint?: number): ChartWindow {
    const layout = chartWindows[selectedRange];
    const serverFrom = source?.from ? Date.parse(source.from) : NaN;
    const from = Number.isFinite(serverFrom)
      ? serverFrom
      : chartWindow(selectedRange, Math.max(Date.now(), (latestPrint ?? 0) + 1)).from;
    return { from, bucketSeconds: layout.bucketSeconds, buckets: layout.buckets };
  }

  function serverBars(source: History | null): KLineData[] {
    return (source?.candles ?? []).flatMap(candle => candle.close === null ? [] : [{
      timestamp: candle.t * 1000,
      open: candle.open ?? candle.close,
      high: candle.high ?? candle.close,
      low: candle.low ?? candle.close,
      close: candle.close,
      volume: candle.volume ?? 0,
    }]);
  }

  /** Real exchange OHLC already folded onto the selected range's grid. */
  function marketBars(source: History | null): KLineData[] {
    return (source?.market_candles?.candles ?? []).flatMap(candle => {
      const values = [candle.open, candle.high, candle.low, candle.close];
      if (!values.every(value => Number.isFinite(value) && value > 0)) return [];
      return [{
        timestamp: candle.t * 1000,
        open: candle.open,
        high: candle.high,
        low: candle.low,
        close: candle.close,
        volume: Number.isFinite(candle.volume) && candle.volume > 0 ? candle.volume : 0,
      }];
    });
  }

  function spotMarks(source: History | null): Mark[] {
    return (source?.spot?.points ?? []).flatMap(point => {
      const time = point.t * 1000, value = Number(point.usd);
      return Number.isFinite(time) && Number.isFinite(value) && value > 0
        ? [{ id: `spot:${point.t}`, time, value }]
        : [];
    }).sort((a, b) => a.time - b.time);
  }

  /**
   * Exchange candles are the primary series when they carry real traded OHLC.
   * Flat zero-volume Gate buckets (common on short ranges) are rejected so the
   * denser spot series can draw instead of white stripes. Desk prints come
   * next, then spot, then publications — all on the selected range's grid.
   */
  function buildLayout(source: History | null, prints: Mark[], window: ChartWindow): { bars: KLineData[]; window: ChartWindow } {
    const market = marketBars(source).filter(bar => (bar.volume ?? 0) > 0);
    if (market.length >= 2) return { bars: market, window };
    const server = serverBars(source);
    if (server.length >= 4) return { bars: server, window };
    const spot = markCandles(spotMarks(source), window);
    if (spot.length) return { bars: spot, window };
    if (server.length) return { bars: server, window };
    return { bars: markCandles(prints, window), window };
  }

  function periodFor(bucketSeconds: number): Period {
    if (bucketSeconds < 60) return { type: "second", span: Math.max(1, bucketSeconds) };
    if (bucketSeconds < 3600) return { type: "minute", span: Math.max(1, Math.round(bucketSeconds / 60)) };
    if (bucketSeconds < 86400) return { type: "hour", span: Math.max(1, Math.round(bucketSeconds / 3600)) };
    if (bucketSeconds < 604800) return { type: "day", span: Math.max(1, Math.round(bucketSeconds / 86400)) };
    return { type: "week", span: Math.max(1, Math.round(bucketSeconds / 604800)) };
  }

  function synchronize(data: KLineData[], pricePrecision: number, window: ChartWindow) {
    if (!chart) return;
    const head = data[0], tail = data[data.length - 1];
    const next = `${data.length}|${head?.timestamp ?? 0}|${head?.open ?? 0}|${tail?.timestamp ?? 0}|${tail?.close ?? 0}|${pricePrecision}|${window.from}|${window.bucketSeconds}|${window.buckets}`;
    if (next === signature) return;
    signature = next;
    chart.setSymbol({ ticker: "EVER/USDT", pricePrecision, volumePrecision: 2 });
    chart.setPeriod(periodFor(window.bucketSeconds));
    chart.resetData();
  }

  function choose(value: ChartRange) {
    if (range === value) return;
    range = value;
    swapping = true;
    try { localStorage.setItem(rangeStorageKey, value); } catch { /* A blocked storage keeps the choice for this visit only. */ }
    history = null;
    signature = "";
    void load().finally(() => { if (range === value) swapping = false; });
  }

  function syncThumb() {
    if (!mounted || !ranges) return;
    const active = rangeButtons[chartRanges.indexOf(range)];
    if (!active) return;
    const border = parseFloat(getComputedStyle(ranges).borderLeftWidth) || 0;
    const offset = Math.round(active.getBoundingClientRect().left - ranges.getBoundingClientRect().left - border);
    if (thumb.ready && thumb.offset === offset && thumb.width === active.offsetWidth) return;
    thumb = { offset, width: active.offsetWidth, ready: true };
  }

  afterUpdate(syncThumb);

  let loadVersion = 0;
  async function load() {
    // A slower response for a previous range must not replace the current one.
    const version = ++loadVersion, requested = range;
    try {
      const next = await getHistory(requested);
      if (version === loadVersion && range === requested) history = next;
    } catch {
      if (version === loadVersion && range === requested) history = null;
    }
  }

  function css(name: string, fallback: string) {
    if (typeof window === "undefined") return fallback;
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
  }

  function buildStyles(): DeepPartial<Styles> {
    const good = css("--color-good", "#8dcb00");
    const danger = css("--color-danger", "var(--otc-sell, #d43d35)");
    const border = css("--color-border", "rgba(41, 67, 42, 0.16)");
    const text = css("--color-text", "#152016");
    // The crosshair chip is painted with the text colour, so its label needs the token that flips with the theme.
    const onText = css("--color-text-invert", "#ffffff");
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
            text: { show: true, style: "fill", color: "#152016", size: 11, family: mono, weight: "500", backgroundColor: null },
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
        horizontal: { show: true, line: { show: true, color: faint, style: "dashed", dashedValue: [3, 3], size: 1 }, text: { show: true, style: "fill", color: onText, size: 11, family: mono, weight: "400", backgroundColor: text } },
        vertical: { show: true, line: { show: true, color: faint, style: "dashed", dashedValue: [3, 3], size: 1 }, text: { show: true, style: "fill", color: onText, size: 11, family: mono, weight: "400", backgroundColor: text } },
      },
    };
  }

  onMount(() => {
    const restored = savedRange();
    if (restored !== range) { range = restored; history = null; signature = ""; }
    chart = init(plot, { locale: "en-US", timezone: "UTC", styles: buildStyles() });
    if (chart) {
      chart.setSymbol({ ticker: "EVER/USDT", pricePrecision: precision, volumePrecision: 2 });
      chart.setPeriod(periodFor(chartBounds(history, range).bucketSeconds));
      chart.createIndicator({ name: "VOL", calcParams: [] });
      chart.setDataLoader({ getBars: ({ callback }) => callback(bars) });
      chart.setOffsetRightDistance(12);
    }
    void load();
    mounted = true;
    syncThumb();
    window.addEventListener("resize", syncThumb);
    document.fonts?.ready.then(syncThumb);
    const timer = setInterval(() => void load(), 60_000);
    const resize = new ResizeObserver(() => chart?.resize());
    resize.observe(plot);
    const theme = new MutationObserver(() => chart?.setStyles(buildStyles()));
    theme.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    return () => { clearInterval(timer); resize.disconnect(); theme.disconnect(); window.removeEventListener("resize", syncThumb); if (chart) dispose(plot); chart = null; };
  });
</script>

<div class="marketChart" class:swapping data-points={marks.length}>
  <div class="chartHead">
    <div class="chartQuote">
      <div class="chartValue">
        <strong data-testid="otc-chart-price">{latest === null ? "—" : `${latest.toLocaleString(numbers, { maximumFractionDigits: Math.max(2, precision) })} $`}</strong>
        {#if change !== null}<em class:negative={change < 0}>{change >= 0 ? "+" : "−"}{Math.abs(change).toFixed(2)}%</em>{/if}
      </div>
      <p class="chartRange" data-testid="otc-chart-range">{say("Candles", "Свечи")} · {candleIntervalLabel(layout.window.bucketSeconds, $locale)}</p>
    </div>
    <div class="ranges" bind:this={ranges} role="group" aria-label={say("Candle timeframe", "Таймфрейм графика")}>
      <span class="rangeThumb" class:ready={thumb.ready} style="transform: translateX({thumb.offset}px); width: {thumb.width}px" aria-hidden="true"></span>
      {#each chartRanges as item, index}<button type="button" bind:this={rangeButtons[index]} class:active={range === item} aria-pressed={range === item} on:click={() => choose(item)}>{item}</button>{/each}
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
  .marketChart { display: flex; width: 100%; min-width: 0; flex: 1; flex-direction: column; border: 1px solid var(--color-border-strong); border-radius: var(--otc-card-radius, 15px); background: var(--exchange-card-bg); overflow: hidden; }
  .chartHead { display: flex; flex-wrap: wrap; align-items: flex-end; justify-content: space-between; gap: 12px; padding: 20px var(--otc-panel-padding, 20px) 8px; }
  .chartQuote { display: grid; gap: 2px; min-width: 0; }
  .chartValue { display: flex; align-items: baseline; gap: 12px; min-width: 0; }
  .chartValue strong { font-family: var(--font-mono); font-size: clamp(34px, 4.4vw, 52px); font-weight: 500; line-height: 1.05; letter-spacing: -.04em; overflow-wrap: anywhere; }
  .chartValue em { color: var(--color-good-ink); font-family: var(--font-mono); font-size: 15px; font-style: normal; }
  .chartValue em.negative { color: var(--color-danger); }
  .chartRange { margin: 0; color: var(--color-text-soft); font-family: var(--font-mono); font-size: 12px; letter-spacing: .02em; }
  .ranges { position: relative; display: flex; flex: 0 1 auto; gap: 2px; padding: 3px; border: 1px solid var(--color-border); border-radius: 999px; background: var(--color-paper); }
  .rangeThumb { position: absolute; z-index: 0; top: 3px; bottom: 3px; left: 0; border-radius: 999px; background: var(--color-good); opacity: 0; pointer-events: none; transition: transform .32s cubic-bezier(.22, 1, .36, 1), width .32s cubic-bezier(.22, 1, .36, 1), opacity .2s ease; }
  .rangeThumb.ready { opacity: 1; }
  .ranges button { position: relative; z-index: 1; min-width: 40px; min-height: 34px; padding: 4px 8px; border: 0; border-radius: 999px; background: transparent; color: var(--color-text-soft); font-family: var(--font-mono); font-size: 12px; line-height: 1.5; transition: color .2s ease; }
  .ranges button:hover { color: var(--color-text); }
  .ranges button.active { color: #152016; font-weight: 650; }
  .chartPlot { position: relative; width: 100%; height: clamp(320px, 34vw, 460px); padding: 0 4px 4px; transition: opacity .22s ease; }
  .marketChart.swapping .chartPlot { opacity: .5; }
  .empty { position: absolute; inset: 0; z-index: 2; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; padding: 24px; text-align: center; pointer-events: none; }
  .empty strong { background: var(--exchange-card-bg); font-size: 14px; font-weight: 650; padding: 5px 12px; }
  .empty p { max-width: 320px; background: var(--exchange-card-bg); color: var(--color-text-soft); font-size: 12px; line-height: 1.6; padding: 5px; }
  @media (max-width: 640px) {
    .chartHead { padding-inline: 12px; }
    /* Ten timeframes do not fit one 320px row, so they tile five per row. */
    .ranges { display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); gap: 4px; width: 100%; border-radius: 14px; }
    .rangeThumb { display: none; }
    .ranges button { min-width: 0; min-height: 32px; padding: 2px; }
    .chartPlot { height: 280px; }
  }
  @media (prefers-reduced-motion: reduce) { .ranges button, .rangeThumb, .chartPlot { transition: none; } }
</style>
