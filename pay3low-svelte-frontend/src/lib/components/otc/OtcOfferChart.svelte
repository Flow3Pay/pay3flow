<script lang="ts">
  import { onMount } from "svelte";
  import { locale } from "$lib/i18n";
  import { rate, type Trade } from "$lib/otc/service";
  import { saleRate, type OfferHistoryPoint } from "$lib/otc/public-offers";
  export let publicOffers: OfferHistoryPoint[] | null = null;
  export let trades: Trade[] = [];
  let plot: HTMLDivElement, plotWidth = 500, plotHeight = 320;
  onMount(() => {
    const observer = new ResizeObserver(([entry]) => { plotWidth = Math.max(1, entry.contentRect.width); plotHeight = Math.max(1, entry.contentRect.height); });
    observer.observe(plot);
    return () => observer.disconnect();
  });
  let range: "1D" | "7D" | "1M" | "ALL" = "ALL";
  const days = { "1D": 1, "7D": 7, "1M": 30, "ALL": Infinity };
  $: say = (en: string, ru: string) => $locale === "ru" ? ru : en;
  // Floating-point conversion is confined to drawing; booking amounts remain exact strings.
  $: source = publicOffers === null
    ? trades.map(t => ({id:t.id, created_at:t.created_at, label:rate(t.terms), direction:t.terms.direction, everAmount:"", usdtAmount:""}))
    : publicOffers.map(o => ({id:o.id, created_at:o.created_at, label:saleRate(o.terms.ever_amount,o.terms.usdt_amount), direction:o.terms.direction, everAmount:o.terms.ever_amount, usdtAmount:o.terms.usdt_amount}));
  $: observations = source.filter(t => t.created_at && Date.now() - Date.parse(t.created_at) <= days[range] * 86400000)
    .map(t => ({ ...t, time:Date.parse(t.created_at!), value:Number(t.label) }))
    .filter(t => Number.isFinite(t.time) && Number.isFinite(t.value) && t.value > 0).sort((a,b) => a.time-b.time);
  $: low = observations.length ? Math.min(...observations.map(t => t.value)) : 0;
  $: high = observations.length ? Math.max(...observations.map(t => t.value)) : 1;
  $: span = high > low ? high - low : Math.max(high * .1, .000000000001);
  $: domainLow = Math.max(0,low-span*.1);
  $: domainHigh = high+span*.1;
  const formatPrice = (value:number) => value.toFixed(12).replace(/\.?0+$/, "");
  $: ticks = [0,.25,.5,.75,1].map(fraction => ({fraction,label:formatPrice(domainHigh-fraction*(domainHigh-domainLow))}));
  $: plotRight = Math.max(48,plotWidth-Math.max(88,...ticks.map(t => t.label.length*7+12)));
  $: x = (time: number) => observations.length < 2 || observations[0].time === observations.at(-1)!.time ? (16+plotRight)/2 : 16 + (time - observations[0].time) / (observations.at(-1)!.time - observations[0].time) * (plotRight - 16);
  $: y = (value: number) => plotHeight - 44 - (value-domainLow)/(domainHigh-domainLow)*(plotHeight-68);
  $: lines = ["buy", "sell"].map(direction => ({ direction, points: observations.filter(t => t.direction === direction).map(t => `${x(t.time)},${y(t.value)}`).join(" ") }));
  let activeId = "";
  $: active = observations.find(point => point.id === activeId) || observations.at(-1);

</script>
<div class="chartTools"><div class="legend"><span class="buyDot"></span>{say("Buy", "Покупка")}<span class="sellDot"></span>{say("Sell", "Продажа")}</div><div class="rangeControls" role="group" aria-label={say("Chart range", "Период графика")}>{#each ["1D", "7D", "1M", "ALL"] as item}<button type="button" class:active={range === item} aria-pressed={range === item} on:click={() => range = item as typeof range}>{item}</button>{/each}</div></div>
<p class="chartLabel">{publicOffers === null ? say("Your desk quotes", "Ваши котировки деска") : say("Published offer history", "История опубликованных предложений")} · USDT / EVER</p>
<div class="chartPlot" bind:this={plot}>
  <svg viewBox={`0 0 ${plotWidth} ${plotHeight}`} role="group" aria-label={publicOffers === null ? say("Private quote history in USDT per EVER", "История приватных котировок в USDT за EVER") : say("Published buy and sell offer prices in USDT per EVER", "Цены опубликованных предложений покупки и продажи в USDT за EVER")}>
    <title>{publicOffers === null ? say("Your server-recorded desk quotes", "Ваши котировки, записанные сервером") : say("Historical published offer prices", "История опубликованных цен предложений")}</title>
    {#each ticks as {fraction,label}}
      <line x1="16" x2={plotRight} y1={24 + fraction * (plotHeight - 68)} y2={24 + fraction * (plotHeight - 68)} class="grid" />
      <line x1={16 + fraction * (plotRight - 16)} x2={16 + fraction * (plotRight - 16)} y1="24" y2={plotHeight - 44} class="grid" />
      {#if observations.length}<text x={plotWidth-8} y={28+fraction*(plotHeight-68)} text-anchor="end" data-testid="otc-chart-price">{label}</text>{/if}
    {/each}
    {#if observations.length}
      {#if publicOffers === null}{#each lines as line}<polyline points={line.points} class:sell={line.direction === "sell"} />{/each}{/if}
      {#each observations as point}<circle cx={x(point.time)} cy={y(point.value)} r={point.id === active?.id ? 6 : 4} class:sell={point.direction === "sell"} tabindex="0" role="button" aria-pressed={point.id === active?.id} aria-label={`${point.direction === "buy" ? say("Buy", "Покупка") : say("Sell", "Продажа")} · ${point.label} USDT / EVER · ${new Date(point.time).toISOString()}`} on:mouseenter={() => activeId=point.id} on:focus={() => activeId=point.id} on:click={() => activeId=point.id} on:keydown={(event) => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); activeId=point.id; } }}><title>{new Date(point.time).toISOString()} · {point.label} USDT / EVER</title></circle>{/each}
      <text x="16" y={plotHeight - 12}>{new Date(observations[0].time).toLocaleDateString()}</text><text x={plotRight} y={plotHeight - 12} text-anchor="end">{new Date(observations.at(-1)!.time).toLocaleDateString()}</text>
    {/if}
  </svg>
  {#if !observations.length}<div class="empty"><strong>{publicOffers === null ? say("No quote history yet", "Истории котировок пока нет") : say("No offer history yet", "Истории предложений пока нет")}</strong><p>{publicOffers === null ? say("Request a desk quote to see its rate here.", "Запросите котировку деска — её курс появится здесь.") : say("Published offers appear here, including after they close.", "Здесь отображаются опубликованные предложения, даже после их закрытия.")}</p></div>{/if}
</div>
{#if active}<div class="chartReadout" data-testid="otc-chart-readout"><strong>{active.direction === "buy" ? say("Buy", "Покупка") : say("Sell", "Продажа")} · {active.label} USDT / EVER</strong>{#if publicOffers !== null}<span>{active.everAmount} EVER · {active.usdtAmount} USDT {say("total", "всего")}</span>{/if}<span>{new Date(active.time).toISOString().replace("T"," ").replace(/\.\d+Z$/, " UTC")}</span></div>{/if}
{#if publicOffers !== null}<p class="chartNote">{say("Each point records the price when an offer was published. Closed offers remain in history.", "Каждая точка фиксирует цену при публикации. Закрытые предложения остаются в истории.")}</p>{/if}
<div class="chartFooter"><span><img src="/icons/assets/pay3flow-mark.svg" width="15" height="15" alt="" />Pay3Flow</span><span>{publicOffers === null ? say("Participant-only", "Только для участников") : say("Offer history", "История предложений")} · UTC</span></div>
<style>
  .chartTools { display: flex; flex: 0 0 auto; min-height: 56px; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 8px 12px; padding: 8px var(--otc-panel-padding, 20px); }
  .legend { display: flex; align-items: center; gap: 6px; color: var(--color-text-soft); font-size: 12px; line-height: 1.5; }
  .buyDot, .sellDot { width: 6px; height: 6px; flex: 0 0 auto; border-radius: 50%; background: var(--otc-buy); }
  .sellDot { margin-left: 8px; background: var(--otc-sell); }
  .rangeControls { display: flex; min-width: 0; gap: 4px; margin-left: auto; }
  button { min-width: 36px; min-height: 36px; padding: 8px; border-radius: 6px; color: var(--color-text-soft); font-family: var(--font-mono); font-size: 12px; line-height: 1.5; }
  button.active { background: var(--color-panel); color: var(--color-text); }
  .chartLabel { padding: 4px var(--otc-panel-padding, 20px) 12px; color: var(--color-text-soft); font-size: 12px; line-height: 1.5; }
  .chartReadout { display:grid; gap:4px; padding:12px var(--otc-panel-padding,20px); border-top:1px solid var(--color-border); font-size:12px; line-height:1.5; overflow-wrap:anywhere; }
  .chartReadout strong { font-family:var(--font-mono); font-weight:500; }
  .chartReadout span, .chartNote { color:var(--color-text-soft); }
  .chartNote { padding:8px var(--otc-panel-padding,20px); font-size:12px; line-height:1.5; }
  circle:focus-visible { outline:2px solid var(--color-focus); }
  .chartPlot { position: relative; flex: 1; min-width: 0; min-height: 280px; }
  svg { position: absolute; inset: 0; display: block; width: 100%; height: 100%; }
  .grid { stroke: var(--color-border); stroke-width: .7; }
  polyline { fill: none; stroke: var(--otc-buy); stroke-width: 2; }
  circle { fill: var(--otc-buy); }
  polyline.sell { stroke: var(--otc-sell); }
  circle.sell { fill: var(--otc-sell); }
  text { fill: var(--color-text-soft); font-family: var(--font-mono); font-size: 12px; }
  .empty { position: absolute; inset: 0; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; padding: 24px; text-align: center; }
  .empty strong { max-width: 100%; padding: 4px 8px; background: var(--exchange-card-bg); font-size: 14px; font-weight: 650; line-height: 1.5; }
  .empty p { max-width: 280px; padding: 4px 8px; background: var(--exchange-card-bg); color: var(--color-text-soft); font-size: 13px; line-height: 1.6; }
  .chartFooter { display: flex; flex: 0 0 auto; min-height: 48px; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 8px 12px; padding: 12px var(--otc-panel-padding, 20px); color: var(--color-text-soft); font-size: 12px; line-height: 1.5; }
  .chartFooter span { display: flex; align-items: center; gap: 6px; }
  @media (max-width: 980px) { .chartPlot { flex: 0 0 auto; height: 320px; } button { min-width: 44px; min-height: 44px; } }
  @media (max-width: 640px) { .rangeControls { display: grid; width: 100%; grid-template-columns: repeat(4, 1fr); margin: 0; } .chartPlot { height: 260px; min-height: 260px; } }
</style>
