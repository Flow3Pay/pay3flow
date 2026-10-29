<script lang="ts">
  import { locale } from "$lib/i18n";
  import { rate, type Trade } from "$lib/otc/service";
  import { bookSummary, candleSeries, chartRanges, compactNumber, filterByRange, percentChange, quoteObservations, rangeBucketMs, trimDecimal, type ChartRange } from "$lib/otc/private-quote-market";
  export let trades: Trade[] = [];
  let range: ChartRange = "ALL";
  let mode: "line" | "candles" = "line";
  let userMode = false;
  let hovered: number | null = null;
  let width = 760, height = 360;
  $: say = (en: string, ru: string) => $locale === "ru" ? ru : en;
  $: allObservations = quoteObservations(trades);
  $: observations = filterByRange(allObservations, range);
  $: if (!userMode) mode = observations.length >= 6 ? "candles" : "line";
  $: candles = candleSeries(observations, rangeBucketMs(range));
  $: latest = observations.at(-1) ?? null;
  $: change = percentChange(observations);
  $: summary = bookSummary(observations);
  $: left = 54;
  $: right = width - 68;
  $: top = 44;
  $: priceBottom = height - 84;
  $: volumeTop = priceBottom + 14;
  $: volumeBottom = height - 30;
  $: plotWidth = Math.max(80, right - left);
  $: allPrices = candles.length ? candles.flatMap(candle => [candle.high, candle.low]) : observations.map(item => item.priceValue);
  $: rawLow = allPrices.length ? Math.min(...allPrices) : 0;
  $: rawHigh = allPrices.length ? Math.max(...allPrices) : 1;
  $: padding = rawHigh > rawLow ? (rawHigh - rawLow) * .16 : Math.max(rawHigh * .02, .000001);
  $: low = Math.max(0, rawLow - padding);
  $: high = rawHigh + padding;
  $: span = high - low || 1;
  $: firstTime = observations[0]?.time ?? 0;
  $: lastTime = observations.at(-1)?.time ?? firstTime;
  $: timeSpan = lastTime - firstTime;
  $: lastPriceY = latest ? y(latest.priceValue) : priceBottom;
  $: maxVolume = Math.max(1, ...candles.map(candle => candle.volume));
  $: points = observations.map((observation, index) => ({ ...observation, x: xFor(index, observation.time), y: y(observation.priceValue) }));
  $: linePath = points.map((point, index) => `${index ? "L" : "M"}${point.x.toFixed(2)},${point.y.toFixed(2)}`).join(" ");
  $: areaPath = points.length ? `${linePath} L${points.at(-1)!.x.toFixed(2)},${priceBottom} L${points[0].x.toFixed(2)},${priceBottom} Z` : "";
  $: hoveredPoint = hovered === null ? null : (mode === "line" ? points[Math.min(hovered, points.length - 1)] : null);
  $: hoveredCandle = hovered === null ? null : (mode === "candles" ? candles[Math.min(hovered, candles.length - 1)] : null);
  $: volume = candles.reduce((sum, candle) => sum + candle.volume, 0);
  $: ohlc = candles.length ? { open: candles[0].open, high: Math.max(...candles.map(candle => candle.high)), low: Math.min(...candles.map(candle => candle.low)), close: candles.at(-1)!.close } : null;
  function y(price: number) { return top + (high - price) / span * (priceBottom - top); }
  function xFor(index: number, time: number) {
    if (observations.length < 2) return left + plotWidth / 2;
    if (timeSpan <= 0) return left + plotWidth * index / Math.max(1, observations.length - 1);
    return left + (time - firstTime) / timeSpan * plotWidth;
  }
  function candleX(index: number) {
    if (candles.length < 2) return left + plotWidth / 2;
    if (candles.at(-1)!.time === candles[0].time) return left + plotWidth * index / Math.max(1, candles.length - 1);
    return left + (candles[index].time - candles[0].time) / (candles.at(-1)!.time - candles[0].time) * plotWidth;
  }
  $: step = plotWidth / Math.max(1, candles.length);
  $: candleWidth = Math.max(2, Math.min(13, step * .56));
  $: priceDecimals = latest ? (latest.priceValue >= 100 ? 2 : latest.priceValue >= 1 ? 3 : latest.priceValue >= .01 ? 5 : 8) : 5;
  function axisPrice(value: number) {
    return value.toLocaleString("en-US", { minimumFractionDigits: priceDecimals, maximumFractionDigits: priceDecimals });
  }
  function axisTime(time: number) {
    const options: Intl.DateTimeFormatOptions = range === "1D"
      ? { hour: "2-digit", minute: "2-digit", timeZone: "UTC" }
      : range === "ALL" ? { day: "2-digit", month: "short", timeZone: "UTC" }
      : { day: "2-digit", month: "short", timeZone: "UTC" };
    return new Date(time).toLocaleString($locale === "ru" ? "ru-RU" : "en-GB", options);
  }
  function fullTime(time: number) {
    return new Date(time).toLocaleString($locale === "ru" ? "ru-RU" : "en-GB", { year: "numeric", month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit", timeZone: "UTC" });
  }
  $: tickCount = width < 520 ? 4 : 6;
  $: ticks = Array.from({ length: tickCount }, (_, index) => {
    const time = observations.length < 2 ? firstTime : firstTime + timeSpan * index / (tickCount - 1);
    return { x: xFor(index, time), time, label: axisTime(time) };
  });
  function hover(event: PointerEvent) {
    if (!observations.length || !(event.currentTarget instanceof Element)) return;
    const rect = event.currentTarget.getBoundingClientRect();
    const svgX = (event.clientX - rect.left) / rect.width * width;
    if (mode === "line") {
      let closest = 0, distance = Infinity;
      points.forEach((point, index) => { const next = Math.abs(point.x - svgX); if (next < distance) { distance = next; closest = index; } });
      hovered = closest;
    } else {
      let closest = 0, distance = Infinity;
      candles.forEach((_, index) => { const next = Math.abs(candleX(index) - svgX); if (next < distance) { distance = next; closest = index; } });
      hovered = closest;
    }
  }
  function chooseRange(value: ChartRange) { range = value; hovered = null; }
  function chooseMode() { userMode = true; mode = mode === "line" ? "candles" : "line"; hovered = null; }
  function observePlot(node: HTMLDivElement) {
    const observer = new ResizeObserver(([entry]) => {
      width = Math.max(280, entry.contentRect.width);
      height = Math.max(width < 560 ? 250 : 300, entry.contentRect.height);
      hovered = null;
    });
    observer.observe(node);
    return { destroy() { observer.disconnect(); } };
  }
</script>

<div class="chartRoot">
<div class="chartTools">
  <div class="chartLegend"><span class="buyDot"></span>{say("Buy quotes", "Котировки покупки")}<span class="sellDot"></span>{say("Sell quotes", "Котировки продажи")}</div>
  <div class="chartButtons">
    <div class="ranges" role="group" aria-label={say("Chart range", "Период графика")}>{#each chartRanges as item}<button type="button" class:active={range === item} aria-pressed={range === item} on:click={() => chooseRange(item)}>{item}</button>{/each}</div>
    <span class="divider"></span>
    <button type="button" class="chartMode" aria-label={mode === "line" ? say("Candlesticks", "Свечи") : say("Price lines", "Линии цены")} title={mode === "line" ? say("Candlesticks", "Свечи") : say("Price lines", "Линии цены")} on:click={chooseMode}>
      <svg width="17" height="17" viewBox="0 0 24 24" fill="none" aria-hidden="true">{#if mode === "line"}<path d="M6 3v18m-3-6h6V8H3v7Zm15-4h-6v6h6v-6ZM18 3v18" stroke="currentColor" stroke-width="1.5" />{:else}<path d="m3 17 5-6 5 3 8-10" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round" />{/if}</svg>
    </button>
  </div>
</div>

{#if latest && ohlc}
  <div class="ohlc">
    <strong>{say("EVER / USDT", "EVER / USDT")}</strong><span class="interval">{range === "1D" ? "5m" : range === "7D" ? "1h" : range === "1M" ? "6h" : "1D"}</span>
    <span>O <b>{axisPrice(ohlc.open)}</b></span><span>H <b>{axisPrice(ohlc.high)}</b></span><span>L <b>{axisPrice(ohlc.low)}</b></span><span class:positive={(change ?? 0) >= 0}>C <b>{axisPrice(ohlc.close)}</b>{#if change !== null}<em class:negative={change < 0}>{change >= 0 ? "+" : ""}{change.toFixed(2)}%</em>{/if}</span>
    <span class="volHead">VOL · {compactNumber(volume)} EVER</span>
  </div>
{:else}
  <div class="ohlc"><strong>{say("EVER / USDT", "EVER / USDT")}</strong><span>{say("Your quote history", "История ваших котировок")}</span></div>
{/if}

<div class="chartPlot" use:observePlot>
  <svg class="priceChart" viewBox={`0 0 ${width} ${height}`} role="img" aria-label={say("Your desk quote history in USDT per EVER", "История котировок деска в USDT за EVER")} on:pointermove={hover} on:pointerleave={() => hovered = null}>
    <title>{say("Participant-only desk quote history", "История котировок деска только для участников")}</title>
    <defs>
      <linearGradient id="otc-quote-fill" x1="0" y1="0" x2="0" y2="1"><stop offset="0%" stop-color={(change ?? 0) >= 0 ? "var(--otc-buy)" : "var(--otc-sell)"} stop-opacity=".16" /><stop offset="100%" stop-color={(change ?? 0) >= 0 ? "var(--otc-buy)" : "var(--otc-sell)"} stop-opacity="0" /></linearGradient>
    </defs>
    {#each [0, 1, 2, 3, 4] as index}
      {@const gridY = top + (priceBottom - top) * index / 4}
      <line class="gridLine" x1={left} x2={right} y1={gridY} y2={gridY} />
      <text class="axisLabel priceLabel" x={right + 12} y={gridY + 4}>{axisPrice(high - span * index / 4)}</text>
    {/each}
    {#each ticks as tick, index}
      <line class="gridLine vertical" x1={tick.x} x2={tick.x} y1={top} y2={volumeBottom} />
      {#if observations.length && (index === 0 || tick.label !== ticks[index - 1].label)}<text class="axisLabel" text-anchor={index === 0 ? "start" : index === ticks.length - 1 ? "end" : "middle"} x={tick.x} y={height - 10}>{tick.label}</text>{/if}
    {/each}

    {#if mode === "line" && points.length}
      <path d={areaPath} fill="url(#otc-quote-fill)" />
      <path class="priceLine" class:negative={(change ?? 0) < 0} d={linePath} />
      {#each points as point}<circle class="quotePoint" class:sell={point.direction === "sell"} cx={point.x} cy={point.y} r="4" />{/each}
    {:else if candles.length}
      {#each candles as candle, index}
        {@const cx = candleX(index)}
        {@const color = candle.close >= candle.open ? "var(--otc-buy)" : "var(--otc-sell)"}
        <line x1={cx} x2={cx} y1={y(candle.high)} y2={y(candle.low)} stroke={color} stroke-width="1.3" />
        <rect x={cx - candleWidth / 2} y={Math.min(y(candle.open), y(candle.close))} width={candleWidth} height={Math.max(2, Math.abs(y(candle.open) - y(candle.close)))} rx="1" fill={color} />
      {/each}
    {/if}

    {#each candles as candle, index}
      {@const barHeight = candle.volume / maxVolume * (volumeBottom - volumeTop)}
      <rect class="volumeBar" x={candleX(index) - candleWidth / 2} y={volumeBottom - barHeight} width={candleWidth} height={barHeight} rx="1" fill={candle.close >= candle.open ? "var(--otc-buy)" : "var(--otc-sell)"} />
    {/each}

    {#if latest}
      <line class="lastPriceLine" class:negative={(change ?? 0) < 0} x1={left} x2={right} y1={lastPriceY} y2={lastPriceY} />
      <rect class="lastPriceBox" class:negative={(change ?? 0) < 0} x={right + 4} y={lastPriceY - 11} width="62" height="22" rx="4" />
      <text class="lastPriceLabel" x={right + 35} y={lastPriceY + 4} text-anchor="middle">{trimDecimal(latest.price, priceDecimals)}</text>
    {/if}

    {#if hovered !== null && hoveredPoint}
      <line class="crosshair" x1={hoveredPoint.x} x2={hoveredPoint.x} y1={top} y2={volumeBottom} />
      <line class="crosshair" x1={left} x2={right} y1={hoveredPoint.y} y2={hoveredPoint.y} />
      <circle class="activePoint" class:sell={hoveredPoint.direction === "sell"} cx={hoveredPoint.x} cy={hoveredPoint.y} r="5" />
    {:else if hovered !== null && hoveredCandle}
      {@const cx = candleX(hovered)}
      <line class="crosshair" x1={cx} x2={cx} y1={top} y2={volumeBottom} />
    {/if}
  </svg>

  {#if !observations.length}
    <div class="empty"><strong>{say("No quote history yet", "Истории котировок пока нет")}</strong><p>{say("Request a desk quote to see its rate here.", "Запросите котировку деска — её курс появится здесь.")}</p></div>
  {/if}

  {#if hoveredPoint}
    <div class="chartTooltip" role="tooltip">
      <strong>{fullTime(hoveredPoint.time)} <small>UTC</small></strong>
      <dl><div><dt>{hoveredPoint.direction === "buy" ? say("Buy quote", "Котировка покупки") : say("Sell quote", "Котировка продажи")}</dt><dd class:sellValue={hoveredPoint.direction === "sell"}>{hoveredPoint.price} <small>USDT</small></dd></div><div><dt>{say("Amount", "Объём")}</dt><dd>{trimDecimal(hoveredPoint.amount, 4)} <small>EVER</small></dd></div><div><dt>{say("Total", "Всего")}</dt><dd>{trimDecimal(hoveredPoint.total, 2)} <small>USDT</small></dd></div></dl>
    </div>
  {:else if hoveredCandle}
    <div class="chartTooltip" role="tooltip">
      <strong>{fullTime(hoveredCandle.time)} <small>UTC</small></strong>
      <dl><div><dt>O / C</dt><dd>{axisPrice(hoveredCandle.open)} / {axisPrice(hoveredCandle.close)}</dd></div><div><dt>H / L</dt><dd>{axisPrice(hoveredCandle.high)} / {axisPrice(hoveredCandle.low)}</dd></div><div><dt>{say("Volume", "Объём")}</dt><dd>{trimDecimal(compactNumber(hoveredCandle.volume), 4)} <small>EVER</small></dd></div></dl>
    </div>
  {/if}
</div>

<div class="chartFooter">
  <span class="source"><img src="/icons/assets/pay3flow-mark.svg" width="15" height="15" alt="" />Pay3Flow · {say("your quotes", "ваши котировки")}</span>
  <span class="marketSummary">
    <span>{say("Spread", "Спред")} <b>{summary.spread === null ? "—" : axisPrice(summary.spread)}</b>{#if summary.spreadPercent !== null}<em>({summary.spreadPercent >= 0 ? "" : ""}{summary.spreadPercent.toFixed(3)}%)</em>{/if}</span>
    <span>{say("Depth", "Глубина")} <b>{compactNumber(summary.bidDepth)} / {compactNumber(summary.askDepth)}</b></span>
    <span>UTC</span>
  </span>
</div>
</div>

<style>
  .chartRoot { display: flex; min-height: 0; flex-direction: column; height: 100%; }
  .chartTools { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 2px 17px 8px; }
  .chartLegend { display: flex; align-items: center; gap: 6px; font-size: 12px; color: var(--color-text-soft); }
  .buyDot, .sellDot { width: 6px; height: 6px; border-radius: 50%; background: var(--otc-buy); }
  .sellDot { background: var(--otc-sell); margin-left: 8px; }
  .chartButtons, .ranges { display: flex; align-items: center; gap: 4px; }
  .ranges button { padding: 5px 8px; border-radius: 5px; font-family: var(--font-mono); font-size: 12px; color: var(--color-text-soft); }
  .ranges button:hover, .ranges button.active { background: var(--color-panel); color: var(--color-text); }
  .ranges button.active { font-weight: 500; }
  .divider { height: 15px; width: 1px; background: var(--color-border); margin: 0 5px; }
  .chartMode { display: grid; width: 26px; height: 26px; place-items: center; color: var(--color-text-soft); border-radius: 5px; }
  .chartMode:hover { background: var(--color-panel); color: var(--color-text); }
  .ohlc { display: flex; flex-wrap: wrap; align-items: center; gap: 9px; min-height: 34px; padding: 0 17px; font-family: var(--font-mono); font-size: 12px; color: var(--color-text-soft); }
  .ohlc strong { color: var(--color-text); font-size: 12px; }
  .ohlc b { font-weight: 400; color: var(--otc-sell); }
  .ohlc .positive b { color: var(--otc-buy); }
  .ohlc em { margin-left: 5px; font-style: normal; color: var(--otc-buy); }
  .ohlc em.negative { color: var(--otc-sell); }
  .ohlc .interval { padding: 2px 5px; border-radius: 4px; background: var(--color-panel); font-size: 10px; }
  .volHead { margin-left: auto; }
  .chartPlot { position: relative; display: flex; flex: 1; min-height: 268px; align-items: stretch; }
  .priceChart { position: absolute; inset: 0; display: block; width: 100%; height: 100%; overflow: visible; font-family: var(--font-mono); touch-action: pan-y; }
  .gridLine { stroke: var(--color-border); stroke-width: .7; }
  .vertical { opacity: .45; }
  .axisLabel { font-size: 11px; fill: var(--color-text-faint); }
  .priceLabel { font-size: 11px; }
  .priceLine { fill: none; stroke: var(--otc-buy); stroke-width: 1.9; stroke-linejoin: round; stroke-linecap: round; }
  .priceLine.negative { stroke: var(--otc-sell); }
  .quotePoint { fill: var(--otc-buy); stroke: var(--color-paper); stroke-width: 1.5; }
  .quotePoint.sell { fill: var(--otc-sell); }
  .volumeBar { opacity: .28; }
  .lastPriceLine { stroke: var(--otc-buy); stroke-width: .8; stroke-dasharray: 3 4; opacity: .7; }
  .lastPriceLine.negative { stroke: var(--otc-sell); }
  .lastPriceBox { fill: var(--otc-buy); }
  .lastPriceBox.negative { fill: var(--otc-sell); }
  .lastPriceLabel { font-size: 10px; fill: #fff; }
  .crosshair { stroke: var(--color-text-soft); stroke-width: .7; stroke-dasharray: 3 3; }
  .activePoint { fill: var(--otc-buy); stroke: var(--color-paper); stroke-width: 2; }
  .activePoint.sell { fill: var(--otc-sell); }
  .chartTooltip { position: absolute; top: 8px; right: 80px; z-index: 5; width: 214px; max-width: calc(100% - 16px); padding: 12px; border: 1px solid var(--color-border-strong); border-radius: 7px; background: var(--color-paper); color: var(--color-text); pointer-events: none; font-family: var(--font-sans); font-size: 12px; box-shadow: 0 4px 16px #0001; }
  .chartTooltip strong { display: block; margin-bottom: 10px; font-size: 12px; font-weight: 650; }
  .chartTooltip dl { display: grid; gap: 8px; margin: 0; }
  .chartTooltip dl > div { display: flex; justify-content: space-between; gap: 8px; }
  .chartTooltip dt { color: var(--color-text-soft); }
  .chartTooltip dd { margin: 0; text-align: right; font-weight: 600; font-variant-numeric: tabular-nums; }
  .chartTooltip dd.sellValue { color: var(--otc-sell); }
  .chartTooltip small { font-size: 10px; color: var(--color-text-soft); }
  .empty { position: absolute; inset: 0; display: flex; flex-direction: column; justify-content: center; align-items: center; gap: 8px; padding: 24px; text-align: center; }
  .empty strong { background: var(--exchange-card-bg); font-size: 14px; padding: 5px 12px; }
  .empty p { max-width: 270px; background: var(--exchange-card-bg); color: var(--color-text-soft); font-size: 12px; line-height: 1.6; padding: 5px; }
  .chartFooter { display: flex; justify-content: space-between; gap: 12px; align-items: center; padding: 8px 17px 14px; color: var(--color-text-soft); font-size: 11px; border-top: 1px solid var(--color-border); margin-top: 2px; }
  .chartFooter > span { display: flex; align-items: center; gap: 6px; }
  .marketSummary { flex-wrap: wrap; gap: 14px !important; }
  .marketSummary b { font-family: var(--font-mono); font-weight: 400; color: var(--color-text); }
  .marketSummary em { font-style: normal; }
  @media (max-width: 640px) {
    .chartTools { padding: 2px 12px 8px; gap: 6px; }
    .chartLegend { font-size: 11px; }
    .ranges button { padding: 6px; }
    .ohlc { padding: 4px 12px; font-size: 11px; gap: 6px; }
    .ohlc .volHead { display: none; }
    .chartPlot { min-height: 230px; }
    .chartFooter { padding: 8px 12px 12px; flex-wrap: wrap; }
    .marketSummary { gap: 10px !important; }
    .chartTooltip { right: 8px; }
  }
</style>
