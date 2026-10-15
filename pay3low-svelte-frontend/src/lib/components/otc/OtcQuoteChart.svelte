<script lang="ts">
  import { onMount } from "svelte";
  import { locale } from "$lib/i18n";
  import { rate, type Trade } from "$lib/otc/service";
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
  $: observations = trades.filter(t => t.created_at && Date.now() - Date.parse(t.created_at) <= days[range] * 86400000)
    .map(t => ({ id: t.id, time: Date.parse(t.created_at!), value: Number(rate(t.terms)), label: rate(t.terms), direction: t.terms.direction }))
    .filter(t => Number.isFinite(t.time) && Number.isFinite(t.value) && t.value > 0).sort((a, b) => a.time - b.time);
  $: low = Math.min(...observations.map(t => t.value));
  $: high = Math.max(...observations.map(t => t.value));
  $: span = high > low ? high - low : Math.max(high * .02, .000001);
  $: x = (time: number) => observations.length < 2 || observations[0].time === observations.at(-1)!.time ? plotWidth / 2 : 24 + (time - observations[0].time) / (observations.at(-1)!.time - observations[0].time) * (plotWidth - 48);
  $: y = (value: number) => plotHeight - 44 - (high === low ? .5 : (value - low + span * .1) / (span * 1.2)) * (plotHeight - 68);
  $: lines = ["buy", "sell"].map(direction => ({ direction, points: observations.filter(t => t.direction === direction).map(t => `${x(t.time)},${y(t.value)}`).join(" ") }));
</script>
<div class="chartTools"><div class="legend"><span class="buyDot"></span>{say("Buy", "Покупка")}<span class="sellDot"></span>{say("Sell", "Продажа")}</div><div class="rangeControls" role="group" aria-label={say("Chart range", "Период графика")}>{#each ["1D", "7D", "1M", "ALL"] as item}<button type="button" class:active={range === item} aria-pressed={range === item} on:click={() => range = item as typeof range}>{item}</button>{/each}</div></div>
<p class="chartLabel">{say("Your desk quotes", "Ваши котировки деска")} · USDT / EVER</p>
<div class="chartPlot" bind:this={plot}>
  <svg viewBox={`0 0 ${plotWidth} ${plotHeight}`} role="img" aria-label={say("Private quote history in USDT per EVER", "История приватных котировок в USDT за EVER")}>
    <title>{say("Your server-recorded desk quotes", "Ваши котировки, записанные сервером")}</title>
    {#each [0, .25, .5, .75, 1] as fraction}<line x1="24" x2={plotWidth - 24} y1={24 + fraction * (plotHeight - 68)} y2={24 + fraction * (plotHeight - 68)} class="grid" /><line x1={24 + fraction * (plotWidth - 48)} x2={24 + fraction * (plotWidth - 48)} y1="24" y2={plotHeight - 44} class="grid" />{/each}
    {#if observations.length}
      {#each lines as line}<polyline points={line.points} class:sell={line.direction === "sell"} />{/each}
      {#each observations as point}<circle cx={x(point.time)} cy={y(point.value)} r="4" class:sell={point.direction === "sell"}><title>{new Date(point.time).toISOString()} · {point.label} USDT / EVER</title></circle>{/each}
      <text x="24" y={plotHeight - 12}>{new Date(observations[0].time).toLocaleDateString()}</text><text x={plotWidth - 24} y={plotHeight - 12} text-anchor="end">{new Date(observations.at(-1)!.time).toLocaleDateString()}</text>
    {/if}
  </svg>
  {#if !observations.length}<div class="empty"><strong>{say("No quote history yet", "Истории котировок пока нет")}</strong><p>{say("Request a desk quote to see its rate here.", "Запросите котировку деска — её курс появится здесь.")}</p></div>{/if}
</div>
<div class="chartFooter"><span><img src="/icons/assets/pay3flow-mark.svg" width="15" height="15" alt="" />Pay3Flow</span><span>{say("Participant-only", "Только для участников")} · UTC</span></div>
<style>
  .chartTools { display: flex; flex: 0 0 auto; min-height: 56px; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 8px 12px; padding: 8px var(--otc-panel-padding, 20px); }
  .legend { display: flex; align-items: center; gap: 6px; color: var(--color-text-soft); font-size: 12px; line-height: 1.5; }
  .buyDot, .sellDot { width: 6px; height: 6px; flex: 0 0 auto; border-radius: 50%; background: var(--otc-buy); }
  .sellDot { margin-left: 8px; background: var(--otc-sell); }
  .rangeControls { display: flex; min-width: 0; gap: 4px; margin-left: auto; }
  button { min-width: 36px; min-height: 36px; padding: 8px; border-radius: 6px; color: var(--color-text-soft); font-family: var(--font-mono); font-size: 12px; line-height: 1.5; }
  button.active { background: var(--color-panel); color: var(--color-text); }
  .chartLabel { padding: 4px var(--otc-panel-padding, 20px) 12px; color: var(--color-text-soft); font-size: 12px; line-height: 1.5; }
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
