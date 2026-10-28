<script lang="ts">
  import { onMount } from "svelte";
  import { locale } from "$lib/i18n";
  import { getHistory, rate, type History, type Trade } from "$lib/otc/service";
  import { saleRate, type OfferHistoryPoint } from "$lib/otc/public-offers";
  import { chartRanges, trimDecimal, type ChartRange } from "$lib/otc/market";

  export let trades: Trade[] = [];
  export let publicOffers: OfferHistoryPoint[] | null = null;

  let range: ChartRange = "1m";
  let history: History | null = null;
  let plot: HTMLDivElement;
  let width = 760, height = 360;
  let from = 0, to = 1;
  let hovered: number | null = null;

  $: say = (en: string, ru: string) => $locale === "ru" ? ru : en;
  $: numbers = $locale === "ru" ? "ru-RU" : "en-US";

  // Participant prints stay as points; the server buckets are the line and volume data.
  $: marks = (publicOffers === null
    ? trades.map(t => ({ id: t.id, at: t.created_at ?? "", price: rate(t.terms), direction: t.terms.direction }))
    : publicOffers.map(o => ({ id: o.id, at: o.created_at, price: saleRate(o.terms.ever_amount, o.terms.usdt_amount), direction: o.terms.direction })))
    .map(mark => ({ ...mark, time: Date.parse(mark.at), value: Number(mark.price) }))
    .filter(mark => Number.isFinite(mark.time) && Number.isFinite(mark.value) && mark.value > 0)
    .sort((a, b) => a.time - b.time);

  $: candles = history?.candles ?? [];
  $: quoteLevels = (history?.levels ?? []).filter(level => level.bid !== null || level.ask !== null);
  $: spine = (history?.spot?.points ?? []).map(point => ({ t: point.t * 1000, value: point.usd }));
  $: bidSeries = quoteLevels.filter(level => level.bid !== null).map(level => ({ t: level.t * 1000, value: level.bid as number }));
  $: askSeries = quoteLevels.filter(level => level.ask !== null).map(level => ({ t: level.t * 1000, value: level.ask as number }));
  $: printSeries = candles.filter(candle => candle.close !== null).map(candle => ({ t: candle.t * 1000, value: candle.close as number, volume: candle.volume as number }));

  $: series = [...bidSeries, ...askSeries, ...printSeries, ...spine, ...marks.map(mark => ({ t: mark.time, value: mark.value }))];
  $: hasSeries = series.length > 0;
  $: timeLow = series.length ? Math.min(...series.map(point => point.t)) : 0;
  $: timeHigh = series.length ? Math.max(...series.map(point => point.t)) : 0;
  $: timeSpan = Math.max(1, timeHigh - timeLow);
  $: priceLow = series.length ? Math.min(...series.map(point => point.value)) : 0;
  $: priceHigh = series.length ? Math.max(...series.map(point => point.value)) : 1;
  $: pricePad = priceHigh > priceLow ? (priceHigh - priceLow) * .12 : Math.max(priceHigh * .05, 1e-9);
  $: domainLow = Math.max(0, priceLow - pricePad);
  $: domainHigh = priceHigh + pricePad;
  $: domainSpan = domainHigh - domainLow || 1;

  $: left = width < 560 ? 46 : 62;
  $: right = Math.max(left + 60, width - (width < 560 ? 44 : 62));
  $: top = 14;
  $: priceBottom = height - (width < 560 ? 66 : 74);
  $: volumeTop = priceBottom + 10;
  $: volumeBottom = height - 26;
  $: windowLeft = left + from * (right - left);
  $: windowRight = left + to * (right - left);
  $: maxVolume = Math.max(1, ...printSeries.map(candle => candle.volume));
  $: decimals = priceHigh >= 100 ? 2 : priceHigh >= 1 ? 3 : priceHigh >= 0.01 ? 5 : 8;
  $: ticks = [0, 1, 2, 3, 4].map(index => ({
    value: domainHigh - domainSpan * index / 4,
    y: top + (priceBottom - top) * index / 4,
  }));
  $: tickCount = width < 520 ? 3 : width < 900 ? 4 : 6;
  $: axisTicks = Array.from({ length: tickCount }, (_, index) => {
    const time = timeLow + timeSpan * index / (tickCount - 1);
    return { x: left + (time - timeLow) / timeSpan * (right - left), label: axisTime(time) };
  });
  $: latestBid = bidSeries.at(-1)?.value ?? null;
  $: latestAsk = askSeries.at(-1)?.value ?? null;
  $: headline = latestBid ?? latestAsk ?? printSeries.at(-1)?.value ?? marks.at(-1)?.value ?? null;
  $: windowStart = timeLow + from * timeSpan;
  $: windowEnd = timeLow + to * timeSpan;
  $: trend = printSeries.length ? printSeries : bidSeries.length ? bidSeries : marks.map(mark => ({ t: mark.time, value: mark.value }));
  $: inWindow = trend.filter(point => point.t >= windowStart && point.t <= windowEnd);
  $: opening = inWindow[0]?.value ?? trend[0]?.value ?? null;
  $: closing = inWindow.at(-1)?.value ?? trend.at(-1)?.value ?? null;
  $: change = opening && closing ? (closing - opening) / opening * 100 : null;
  $: active = hovered === null ? marks.at(-1) ?? null : marks[Math.min(hovered, marks.length - 1)];

  function y(price: number) { return top + (domainHigh - price) / domainSpan * (priceBottom - top); }
  function x(time: number) { return left + (time - timeLow) / timeSpan * (right - left); }
  function path(points: { t: number; value: number }[]) {
    return points.map((point, index) => `${index ? "L" : "M"}${x(point.t).toFixed(1)},${y(point.value).toFixed(1)}`).join(" ");
  }
  function axisPrice(value: number) {
    return value.toLocaleString(numbers, { minimumFractionDigits: decimals, maximumFractionDigits: decimals });
  }
  function axisTime(time: number) {
    const detail = range === "1h" || range === "24h";
    return new Date(time).toLocaleString(numbers, detail
      ? { hour: "2-digit", minute: "2-digit", timeZone: "UTC" }
      : { day: "2-digit", month: "short", timeZone: "UTC" });
  }
  function fullTime(time: number) {
    return new Date(time).toLocaleString(numbers, { year: "numeric", month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit", timeZone: "UTC" });
  }
  function choose(value: ChartRange) { range = value; hovered = null; void load(); }
  function shiftWindow(edge: "from" | "to", raw: string) {
    const fraction = Math.min(1, Math.max(0, Number(raw) / 1000));
    if (edge === "from") from = Math.min(fraction, to - .02);
    else to = Math.max(fraction, from + .02);
  }
  function hover(event: PointerEvent) {
    if (!marks.length || !(event.target instanceof SVGElement)) return;
    const root = event.currentTarget instanceof Element ? event.currentTarget.querySelector("svg") : null;
    if (!(root instanceof SVGSVGElement)) return;
    const rect = root.getBoundingClientRect();
    const svgX = (event.clientX - rect.left) / rect.width * width;
    let closest = 0, distance = Infinity;
    marks.forEach((mark, index) => {
      const next = Math.abs(x(mark.time) - svgX);
      if (next < distance) { distance = next; closest = index; }
    });
    hovered = closest;
  }
  async function load() {
    try { history = await getHistory(range); } catch { history = null; }
  }
  onMount(() => {
    void load();
    const timer = setInterval(() => void load(), 60_000);
    const observer = new ResizeObserver(([entry]) => {
      width = Math.max(280, entry.contentRect.width);
      height = Math.max(entry.contentRect.width < 560 ? 250 : 300, entry.contentRect.height);
      hovered = null;
    });
    observer.observe(plot);
    return () => { clearInterval(timer); observer.disconnect(); };
  });
</script>

<div class="marketChart">
  <div class="chartHead">
    <div class="chartQuote">
      <span class="chartPair">EVER / USDT</span>
      <div class="chartValue">
        <strong>{headline === null ? "—" : `$${trimDecimal(axisPrice(headline), decimals)}`}</strong>
        {#if change !== null}<em class:negative={change < 0}>{change >= 0 ? "+" : "−"}{Math.abs(change).toFixed(2)}%</em>{/if}
      </div>
    </div>
    <div class="ranges" role="group" aria-label={say("Chart range", "Период графика")}>
      {#each chartRanges as item}<button type="button" class:active={range === item} aria-pressed={range === item} on:click={() => choose(item)}>{item}</button>{/each}
    </div>
  </div>

  <div class="chartPlot" role="presentation" bind:this={plot} on:pointermove={hover} on:pointerleave={() => hovered = null}>
    <svg viewBox={`0 0 ${width} ${height}`} role="img" aria-label={say("EVER price history in USDT per EVER", "История цены EVER в USDT за EVER")}>
      <title>{say("Market price history in USDT per EVER", "История цены рынка в USDT за EVER")}</title>
      <defs>
        <linearGradient id="otc-volume-fill" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stop-color="var(--color-accent)" stop-opacity=".22" />
          <stop offset="100%" stop-color="var(--color-accent)" stop-opacity="0" />
        </linearGradient>
        <clipPath id="otc-window"><rect x={windowLeft} y={0} width={Math.max(0, windowRight - windowLeft)} height={height} /></clipPath>
        <clipPath id="otc-outside">
          <rect x={0} y={0} width={Math.max(0, windowLeft)} height={height} />
          <rect x={windowRight} y={0} width={Math.max(0, width - windowRight)} height={height} />
        </clipPath>
      </defs>

      {#each ticks as tick}
        <line class="gridLine" x1={left} x2={right} y1={tick.y} y2={tick.y} />
        {#if hasSeries}<text class="axisLabel priceLabel" data-testid="otc-chart-price" x={left - 8} y={tick.y + 4} text-anchor="end">{axisPrice(tick.value)}</text>{/if}
      {/each}
      {#each axisTicks as tick}
        <text class="axisLabel" x={tick.x} y={height - 8} text-anchor="middle">{tick.label}</text>
      {/each}

      {#if printSeries.length}
        <g clip-path="url(#otc-window)">
          {#each printSeries as candle}
            {@const barHeight = Math.min(1, candle.volume / maxVolume) * (volumeBottom - volumeTop)}
            <rect class="volumeBar" x={x(candle.t) - 3} y={volumeBottom - barHeight} width="6" height={barHeight} rx="1" />
          {/each}
        </g>
      {/if}

      {#if spine.length > 1}
        <path class="historyLine" d={path(spine)} clip-path="url(#otc-outside)" />
        <path class="historyLine activeLine" d={path(spine)} clip-path="url(#otc-window)" />
      {/if}
      {#if bidSeries.length > 1}<path class="bidLine" d={path(bidSeries)} clip-path="url(#otc-outside)" /><path class="bidLine activeLine" d={path(bidSeries)} clip-path="url(#otc-window)" />{/if}
      {#if askSeries.length > 1}<path class="askLine" d={path(askSeries)} clip-path="url(#otc-outside)" /><path class="askLine activeLine" d={path(askSeries)} clip-path="url(#otc-window)" />{/if}

      {#each marks as mark, index}
        <circle class="quotePoint" class:sell={mark.direction === "sell"} cx={x(mark.time)} cy={y(mark.value)} r={index === hovered ? 6 : 4} />
      {/each}

      {#if hovered !== null && active}
        <line class="crosshair" x1={x(active.time)} x2={x(active.time)} y1={top} y2={volumeBottom} />
      {/if}

      {#if latestBid !== null}
        <g class="endTag bidTag" transform={`translate(${right + 4},${Math.min(priceBottom - 11, Math.max(top, y(latestBid) - 10))})`}>
          <rect width="54" height="20" rx="5" /><text x="27" y="14" text-anchor="middle">{trimDecimal(axisPrice(latestBid), decimals)}</text>
        </g>
      {/if}
      {#if latestAsk !== null}
        <g class="endTag askTag" transform={`translate(${right + 4},${Math.min(priceBottom - 11, Math.max(top, y(latestAsk) - 10))})`}>
          <rect width="54" height="20" rx="5" /><text x="27" y="14" text-anchor="middle">{trimDecimal(axisPrice(latestAsk), decimals)}</text>
        </g>
      {/if}
    </svg>

    {#if !hasSeries && !marks.length}
      <div class="empty">
        <strong>{publicOffers === null ? say("No quote history yet", "Истории котировок пока нет") : say("No offer history yet", "Истории предложений пока нет")}</strong>
        <p>{say("Prices appear here as the desk quotes and offers publish.", "Здесь появятся цены котировок деска и опубликованных предложений.")}</p>
      </div>
    {/if}
  </div>

  <div class="brush">
    <div class="brushTrack" aria-hidden="true">
      <span class="brushPre" style={`width:${from * 100}%`}></span>
      <span class="brushPost" style={`left:${to * 100}%`}></span>
      <span class="brushWindow" style={`left:${from * 100}%;width:${(to - from) * 100}%`}></span>
    </div>
    <div class="brushHandles">
      <input type="range" min="0" max="1000" value={Math.round(from * 1000)} aria-label={say("Window start", "Начало окна")} on:input={(event) => shiftWindow("from", event.currentTarget.value)} />
      <input type="range" min="0" max="1000" value={Math.round(to * 1000)} aria-label={say("Window end", "Конец окна")} on:input={(event) => shiftWindow("to", event.currentTarget.value)} />
    </div>
    <p class="brushReadout">{fullTime(windowStart).replace(/.*,\s/, "")} — {fullTime(windowEnd).replace(/.*,\s/, "")}</p>
  </div>

  {#if active}
    <div class="chartReadout" data-testid="otc-chart-readout">
      <strong>{active.direction === "buy" ? say("Buy", "Покупка") : say("Sell", "Продажа")} · {active.price} USDT / EVER</strong>
      <span>{fullTime(active.time)} UTC</span>
    </div>
  {/if}

  <div class="chartFooter">
    <span class="legend"><i class="bidDot"></i>{say("Bids", "Покупки")}<i class="askDot"></i>{say("Asks", "Продажи")}</span>
    <span>{history?.spot?.source ?? "Pay3Flow"}{history?.spot?.stale ? say(" · cached", " · кэш") : ""} · UTC</span>
  </div>
</div>

<style>
  .marketChart { display: flex; min-height: 0; flex: 1; flex-direction: column; }
  .chartHead { display: flex; flex-wrap: wrap; align-items: flex-end; justify-content: space-between; gap: 12px; padding: 4px var(--otc-panel-padding, 20px) 10px; }
  .chartQuote { display: grid; gap: 2px; min-width: 0; }
  .chartPair { color: var(--color-text-soft); font-family: var(--font-mono); font-size: 12px; letter-spacing: .04em; text-transform: uppercase; }
  .chartValue { display: flex; align-items: baseline; gap: 12px; min-width: 0; }
  .chartValue strong { font-family: var(--font-mono); font-size: clamp(34px, 4.4vw, 56px); font-weight: 500; line-height: 1.05; letter-spacing: -.04em; overflow-wrap: anywhere; }
  .chartValue em { color: var(--otc-buy); font-family: var(--font-mono); font-size: 15px; font-style: normal; }
  .chartValue em.negative { color: var(--otc-sell); }
  .ranges { display: flex; flex: 0 1 auto; gap: 2px; padding: 3px; border: 1px solid var(--color-border); border-radius: 999px; background: var(--color-paper); }
  .ranges button { min-width: 40px; min-height: 34px; padding: 4px 8px; border-radius: 999px; color: var(--color-text-soft); font-family: var(--font-mono); font-size: 12px; line-height: 1.5; transition: background .18s ease, color .18s ease; }
  .ranges button:hover { color: var(--color-text); }
  .ranges button.active { background: var(--color-panel); color: var(--color-text); font-weight: 650; }
  .chartPlot { position: relative; display: flex; flex: 1; min-height: 260px; align-items: stretch; padding: 0 6px; }
  .chartPlot svg { position: absolute; inset: 0; display: block; width: 100%; height: 100%; overflow: visible; font-family: var(--font-mono); touch-action: pan-y; }
  .gridLine { stroke: var(--color-border); stroke-width: .7; }
  .axisLabel { fill: var(--color-text-faint); font-size: 11px; }
  .priceLabel { font-size: 10px; }
  .volumeBar { fill: url(#otc-volume-fill); }
  .historyLine { fill: none; stroke: var(--color-border-strong); stroke-width: 1.4; }
  .historyLine.activeLine { stroke: var(--color-text-faint); }
  .bidLine { fill: none; stroke: var(--otc-buy); stroke-width: 2; stroke-linecap: round; stroke-linejoin: round; opacity: .55; }
  .askLine { fill: none; stroke: var(--otc-sell); stroke-width: 2; stroke-linecap: round; stroke-linejoin: round; opacity: .55; }
  .activeLine { opacity: 1; }
  .quotePoint { fill: var(--otc-buy); stroke: var(--exchange-card-bg); stroke-width: 1.5; }
  .quotePoint.sell { fill: var(--otc-sell); }
  .crosshair { stroke: var(--color-text-soft); stroke-width: .7; stroke-dasharray: 3 3; }
  .endTag rect { stroke-width: 1; }
  .endTag text { font-size: 10px; }
  .bidTag rect { fill: var(--otc-buy-soft); stroke: var(--otc-buy); }
  .bidTag text { fill: var(--otc-buy); }
  .askTag rect { fill: var(--otc-sell-soft); stroke: var(--otc-sell); }
  .askTag text { fill: var(--otc-sell); }
  .empty { position: absolute; inset: 0; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; padding: 24px; text-align: center; }
  .empty strong { background: var(--exchange-card-bg); font-size: 14px; font-weight: 650; padding: 5px 12px; }
  .empty p { max-width: 300px; background: var(--exchange-card-bg); color: var(--color-text-soft); font-size: 12px; line-height: 1.6; padding: 5px; }
  .brush { position: relative; display: grid; gap: 6px; padding: 8px var(--otc-panel-padding, 20px) 2px; }
  .brushTrack { position: relative; height: 26px; border: 1px solid var(--color-border); border-radius: 8px; background: var(--color-panel); overflow: hidden; }
  .brushPre, .brushPost, .brushWindow { position: absolute; top: 0; bottom: 0; }
  .brushPre { left: 0; background: var(--color-paper); opacity: .7; }
  .brushPost { right: 0; background: var(--color-paper); opacity: .7; }
  .brushWindow { border-inline: 2px solid var(--color-text); background: var(--color-accent-soft); }
  .brushHandles { position: absolute; inset: 8px var(--otc-panel-padding, 20px) auto; display: block; height: 26px; pointer-events: none; }
  .brushHandles input { position: absolute; inset: 0; width: 100%; height: 26px; margin: 0; -webkit-appearance: none; appearance: none; background: transparent; pointer-events: none; }
  .brushHandles input::-webkit-slider-thumb { -webkit-appearance: none; appearance: none; width: 14px; height: 24px; border: 1px solid var(--color-text); border-radius: 5px; background: var(--color-panel-strong, var(--color-panel)); cursor: ew-resize; pointer-events: auto; }
  .brushHandles input::-moz-range-thumb { width: 14px; height: 24px; border: 1px solid var(--color-text); border-radius: 5px; background: var(--color-panel); cursor: ew-resize; pointer-events: auto; }
  .brushHandles input:focus-visible { outline: var(--focus-ring-width) solid var(--color-focus); outline-offset: 2px; }
  .brushReadout { color: var(--color-text-faint); font-family: var(--font-mono); font-size: 11px; line-height: 1.5; }
  .chartReadout { display: grid; gap: 2px; padding: 10px var(--otc-panel-padding, 20px); border-top: 1px solid var(--color-border); font-size: 12px; line-height: 1.5; overflow-wrap: anywhere; }
  .chartReadout strong { font-family: var(--font-mono); font-weight: 500; }
  .chartReadout span { color: var(--color-text-soft); }
  .chartFooter { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 8px 12px; padding: 8px var(--otc-panel-padding, 20px) 14px; border-top: 1px solid var(--color-border); color: var(--color-text-soft); font-size: 11px; line-height: 1.5; }
  .legend { display: flex; align-items: center; gap: 6px; }
  .bidDot, .askDot { width: 14px; height: 3px; border-radius: 2px; background: var(--otc-buy); }
  .askDot { margin-left: 6px; background: var(--otc-sell); }
  @media (max-width: 640px) {
    .chartHead { padding-inline: 12px; }
    .ranges { width: 100%; justify-content: space-between; }
    .ranges button { min-width: 0; flex: 1; }
    .chartPlot { min-height: 230px; }
    .brush, .chartReadout { padding-inline: 12px; }
    .brushHandles { inset-inline: 12px; }
    .chartFooter { padding-inline: 12px; }
  }
  @media (prefers-reduced-motion: reduce) { .ranges button { transition: none; } }
</style>
