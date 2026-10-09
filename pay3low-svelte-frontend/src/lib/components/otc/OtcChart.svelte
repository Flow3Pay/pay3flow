<script lang="ts">
  import { onMount } from "svelte";
  import { locale } from "$lib/i18n";
  import { otcCopy } from "$lib/otc/copy";
  import { demoCandles, formatPrice, type OtcMarket, type ChartRange } from "$lib/otc/model";
  export let market: OtcMarket;
  let range: ChartRange = "1D";
  let mode: "lines" | "candles" = "lines";
  let zoom = 1;
  let hovered: number | null = null;
  let plot: HTMLDivElement;
  let width = 680, height = 375;
  const left = 12, top = 24;
  $: right = width - 82;
  $: bottom = height - 107;
  $: volumeBottom = height - 40;
  onMount(() => {
    const observer = new ResizeObserver(([entry]) => {
      width = Math.max(280, entry.contentRect.width);
      height = width < 500 ? 290 : 375;
      hovered = null;
    });
    observer.observe(plot);
    return () => observer.disconnect();
  });
  const ranges: ChartRange[] = ["1D", "7D", "1M", "1Y"];
  $: copy = otcCopy($locale);
  $: allCandles = demoCandles(market, range);
  $: candles = allCandles.slice(-Math.round(allCandles.length / zoom));
  $: minimum = Math.min(...candles.map((candle) => candle.low)) - market.price * .004;
  $: maximum = Math.max(...candles.map((candle) => candle.high)) + market.price * .004;
  $: maximumVolume = Math.max(...candles.map((candle) => candle.volume));
  $: step = (right - left) / candles.length;
  $: active = candles[hovered === null ? candles.length - 1 : Math.min(hovered, candles.length - 1)];
  $: buyLine = line(-market.price * .0015);
  $: sellLine = line(market.price * .0015);
  $: lastY = y(market.price);
  $: x = (index: number) => left + step * (index + .5);
  $: y = (price: number) => top + (maximum - price) / (maximum - minimum) * (bottom - top);
  $: line = (offset: number) => candles.map((candle, index) => `${index ? "L" : "M"}${x(index).toFixed(2)},${y(candle.close + offset).toFixed(2)}`).join(" ");
  function hover(event: PointerEvent) {
    const rect = event.currentTarget instanceof Element ? event.currentTarget.getBoundingClientRect() : null;
    if (!rect) return;
    const svgX = (event.clientX - rect.left) / rect.width * width;
    hovered = Math.max(0, Math.min(candles.length - 1, Math.floor((svgX - left) / step)));
  }
  function dateLabel(time: number) {
    return new Date(time).toLocaleString($locale === "ru" ? "ru-RU" : "en-GB", range === "1D" ? { hour: "2-digit", minute: "2-digit", timeZone: "UTC" } : { day: "numeric", month: "short", timeZone: "UTC" });
  }
  function changeRange(value: ChartRange) { range = value; hovered = null; zoom = 1; }
</script>
<div class="chartTools">
  <div class="chartLegend"><span class="buyDot"></span>{copy.buy}<span class="sellDot"></span>{copy.sell}</div>
  <div class="chartButtons"><div class="ranges" role="group" aria-label={copy.chartRange}>{#each ranges as item}<button type="button" class:active={range === item} aria-pressed={range === item} on:click={() => changeRange(item)}>{item}</button>{/each}</div><span class="divider"></span><button type="button" class="chartMode" aria-label={mode === "lines" ? copy.candles : copy.lines} title={mode === "lines" ? copy.candles : copy.lines} on:click={() => mode = mode === "lines" ? "candles" : "lines"}><svg width="17" height="17" viewBox="0 0 24 24" fill="none" aria-hidden="true">{#if mode === "lines"}<path d="M6 3v18m-3-6h6V8H3v7Zm15-4h-6v6h6v-6ZM18 3v18" stroke="currentColor" stroke-width="1.5" />{:else}<path d="m3 17 5-6 5 3 8-10" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round" />{/if}</svg></button></div>
</div>
<div class="ohlc"><strong>{market.base}/{market.quote}</strong><span>{range === "1D" ? "15m" : range === "7D" ? "2h" : range === "1M" ? "12h" : "6d"}</span><span>O <b>{formatPrice(active.open, market)}</b></span><span>H <b>{formatPrice(active.high, market)}</b></span><span>L <b>{formatPrice(active.low, market)}</b></span><span class:positive={active.close >= active.open}>C <b>{formatPrice(active.close, market)}</b></span></div>
<div class="chartPlot" bind:this={plot}>
  <svg class="priceChart" viewBox={`0 0 ${width} ${height}`} role="img" aria-label={copy.chartDescription} on:pointermove={hover} on:pointerleave={() => hovered = null}>
    <title>{copy.chartDescription} · {market.base}/{market.quote} · {copy.demo}</title>
    <defs><linearGradient id="otc-price-fill" x1="0" y1="0" x2="0" y2="1"><stop offset="0%" stop-color="var(--otc-buy)" stop-opacity=".14" /><stop offset="100%" stop-color="var(--otc-buy)" stop-opacity="0" /></linearGradient></defs>
    {#each [0, 1, 2, 3, 4] as index}
      {@const gridY = top + (bottom - top) * index / 4}
      <line class="gridLine" x1={left} x2={right} y1={gridY} y2={gridY} /><text class="axisLabel" x={right + 12} y={gridY + 4}>{formatPrice(maximum - (maximum - minimum) * index / 4, market)}</text>
    {/each}
    {#each (width < 500 ? [0, 2, 4, 5] : [0, 1, 2, 3, 4, 5]) as index}
      {@const candleIndex = Math.round((candles.length - 1) * index / 5)}
      <line class="gridLine vertical" x1={x(candleIndex)} x2={x(candleIndex)} y1={top} y2={volumeBottom} />
      <text class="axisLabel dateLabel" text-anchor={index === 0 ? "start" : index === 5 ? "end" : "middle"} x={x(candleIndex)} y={height - 15}>{dateLabel(candles[candleIndex].time)}</text>
    {/each}
    {#if mode === "lines"}
      <path d={`${buyLine} L${x(candles.length - 1)},${bottom} L${x(0)},${bottom} Z`} fill="url(#otc-price-fill)" />
      <path class="buyLine" d={buyLine} /><path class="sellLine" d={sellLine} />
    {:else}
      {#each candles as candle, index}
        {@const color = candle.close >= candle.open ? "var(--otc-buy)" : "var(--otc-sell)"}
        <line x1={x(index)} x2={x(index)} y1={y(candle.high)} y2={y(candle.low)} stroke={color} stroke-width="1.3" />
        <rect x={x(index) - step * .29} y={Math.min(y(candle.open), y(candle.close))} width={step * .58} height={Math.max(1.5, Math.abs(y(candle.open) - y(candle.close)))} rx=".8" fill={color} />
      {/each}
    {/if}
    <line class="lastPriceLine" x1={left} x2={right} y1={lastY} y2={lastY} />
    <rect x={right + 4} y={lastY - 11} width="80" height="23" rx="4" fill="var(--otc-buy)" /><text class="lastPriceLabel" x={right + 44} y={lastY + 4} text-anchor="middle">{formatPrice(market.price, market)}</text>
    <text class="volumeLabel" x={left + 4} y={height - 82}>VOL · {Math.round(active.volume).toLocaleString("en-US")}</text>
    {#each candles as candle, index}
      {@const barHeight = candle.volume / maximumVolume * 33}
      <rect x={x(index) - step * .32} y={volumeBottom - barHeight} width={step * .64} height={barHeight} rx="1" fill={candle.close >= candle.open ? "var(--otc-buy)" : "var(--otc-sell)"} opacity=".35" />
    {/each}
    {#if hovered !== null}
      <line class="crosshair" x1={x(hovered)} x2={x(hovered)} y1={top} y2={volumeBottom} />
      <line class="crosshair" x1={left} x2={right} y1={y(active.close)} y2={y(active.close)} />
      <circle cx={x(hovered)} cy={y(active.close)} r="4" fill="var(--otc-buy)" stroke="var(--color-paper)" stroke-width="2" />
      <rect x={Math.min(right - 84, Math.max(left, x(hovered) - 42))} y={height - 32} width="84" height="24" rx="4" fill="var(--color-text)" /><text x={Math.min(right - 42, Math.max(left + 42, x(hovered)))} y={height - 16} text-anchor="middle" fill="var(--color-paper)" font-size="10">{dateLabel(active.time)}</text>
    {/if}
  </svg>
</div>
<div class="chartFooter"><span><img src="/icons/assets/pay3flow_logo.svg" width="15" height="15" alt="" />Pay3Flow <span class="footerDemo">/ {copy.demo}</span></span><div class="zoomTools"><button type="button" aria-label={copy.zoomOut} disabled={zoom === 1} on:click={() => { zoom = Math.max(1, zoom - .5); hovered = null; }}>−</button><button type="button" aria-label={copy.chartReset} on:click={() => { zoom = 1; hovered = null; }}>↺</button><button type="button" aria-label={copy.zoomIn} disabled={zoom === 3} on:click={() => { zoom = Math.min(3, zoom + .5); hovered = null; }}>+</button></div><span class="timezone">UTC</span></div>
<style>
  .chartTools { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 16px 17px 8px; }
  .chartLegend { display: flex; align-items: center; gap: 6px; font-size: 10px; color: var(--color-text-soft); }
  .buyDot, .sellDot { width: 6px; height: 6px; border-radius: 50%; background: var(--otc-buy); }
  .sellDot { background: var(--otc-sell); margin-left: 7px; }
  .chartButtons, .ranges { display: flex; align-items: center; gap: 4px; }
  .ranges button { padding: 5px 8px; border-radius: 5px; font-family: var(--font-mono); font-size: 10px; color: var(--color-text-soft); }
  .ranges button:hover, .ranges button.active { background: var(--color-panel); color: var(--color-text); }
  .ranges button.active { font-weight: 500; }
  .divider { height: 15px; width: 1px; background: var(--color-border); margin: 0 5px; }
  .chartMode { display: grid; width: 26px; height: 26px; place-items: center; color: var(--color-text-soft); }
  .ohlc { display: flex; flex-wrap: wrap; align-items: center; gap: 9px; min-height: 36px; padding: 0 17px; font-family: var(--font-mono); font-size: 9px; color: var(--color-text-soft); }
  .ohlc strong { color: var(--color-text); font-size: 10px; }
  .ohlc b { font-weight: 400; color: var(--otc-sell); }
  .ohlc .positive b { color: var(--otc-buy); }
  .chartPlot { min-height: 290px; display: flex; align-items: stretch; }
  .priceChart { display: block; width: 100%; min-height: 290px; overflow: visible; font-family: var(--font-mono); }
  .gridLine { stroke: var(--color-border); stroke-width: .7; }
  .vertical { opacity: .5; }
  .axisLabel, .volumeLabel { font-size: 10px; fill: var(--color-text-faint); }
  .volumeLabel { font-size: 9px; }
  .buyLine, .sellLine { fill: none; stroke: var(--otc-buy); stroke-width: 1.9; stroke-linejoin: round; stroke-linecap: round; }
  .sellLine { stroke: var(--otc-sell); stroke-width: 1.9; }
  .lastPriceLine { stroke: var(--otc-buy); stroke-width: .8; stroke-dasharray: 3 4; opacity: .7; }
  .lastPriceLabel { font-size: 10px; fill: #fff; }
  .crosshair { stroke: var(--color-text-soft); stroke-width: .7; stroke-dasharray: 3 3; }
  .chartFooter { display: flex; align-items: center; justify-content: space-between; padding: 3px 17px 13px; font-size: 10px; color: var(--color-text-soft); }
  .chartFooter > span { display: flex; align-items: center; gap: 6px; }
  .footerDemo { color: var(--color-text-faint); font-size: 9px; }
  .zoomTools { display: flex; gap: 3px; border: 1px solid var(--color-border); border-radius: 6px; padding: 2px; }
  .zoomTools button { width: 25px; height: 22px; color: var(--color-text-soft); font-size: 14px; }
  .zoomTools button:hover { background: var(--color-panel); border-radius: 3px; }
  .zoomTools button:disabled { opacity: .35; cursor: default; }
  .timezone { font-family: var(--font-mono); font-size: 9px; }
  @media (max-width: 640px) {
    .chartTools { padding: 12px 12px 4px; gap: 6px; }
    .chartLegend { font-size: 9px; }
    .ranges button { padding: 6px; }
    .ohlc { padding: 5px 12px; font-size: 8px; gap: 6px; }
    .ohlc > span:nth-child(4), .ohlc > span:nth-child(5) { display: none; }
    .chartPlot, .priceChart { min-height: 235px; }
    .chartFooter { padding: 0 12px 12px; }
    .footerDemo { display: none; }
  }
</style>
