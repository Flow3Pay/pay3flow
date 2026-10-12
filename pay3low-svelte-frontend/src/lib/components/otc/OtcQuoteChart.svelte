<script lang="ts">
  import { locale } from "$lib/i18n";
  import { rate, type Trade } from "$lib/otc/service";
  export let trades: Trade[] = [];
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
  $: x = (time: number) => observations.length < 2 || observations[0].time === observations.at(-1)!.time ? 250 : 24 + (time - observations[0].time) / (observations.at(-1)!.time - observations[0].time) * 420;
  $: y = (value: number) => 270 - (value - low + span * .1) / (span * 1.2) * 230;
  $: lines = ["buy", "sell"].map(direction => ({ direction, points: observations.filter(t => t.direction === direction).map(t => `${x(t.time)},${y(t.value)}`).join(" ") }));
</script>
<div class="chartTools"><div class="legend"><span class="buyDot"></span>{say("Buy", "Покупка")}<span class="sellDot"></span>{say("Sell", "Продажа")}</div><div role="group" aria-label={say("Chart range", "Период графика")}>{#each ["1D", "7D", "1M", "ALL"] as item}<button type="button" class:active={range === item} aria-pressed={range === item} on:click={() => range = item as typeof range}>{item}</button>{/each}</div></div>
<p class="chartLabel">{say("Your desk quotes", "Ваши котировки деска")} · USDT / EVER</p>
<div class="chartPlot">
  <svg viewBox="0 0 500 340" role="img" aria-label={say("Private quote history in USDT per EVER", "История приватных котировок в USDT за EVER")}>
    <title>{say("Your server-recorded desk quotes", "Ваши котировки, записанные сервером")}</title>
    {#each [40, 100, 160, 220, 280] as line}<line x1="24" x2="450" y1={line} y2={line} class="grid" />{/each}
    {#each [24, 130, 236, 342, 450] as line}<line x1={line} x2={line} y1="40" y2="280" class="grid" />{/each}
    {#if observations.length}
      {#each lines as line}<polyline points={line.points} class:sell={line.direction === "sell"} />{/each}
      {#each observations as point}<circle cx={x(point.time)} cy={y(point.value)} r="4" class:sell={point.direction === "sell"}><title>{new Date(point.time).toISOString()} · {point.label} USDT / EVER</title></circle>{/each}
      <text x="24" y="320">{new Date(observations[0].time).toLocaleDateString()}</text><text x="450" y="320" text-anchor="end">{new Date(observations.at(-1)!.time).toLocaleDateString()}</text>
    {/if}
  </svg>
  {#if !observations.length}<div class="empty"><strong>{say("No quote history yet", "Истории котировок пока нет")}</strong><p>{say("Request a desk quote to see its rate here.", "Запросите котировку деска — её курс появится здесь.")}</p></div>{/if}
</div>
<div class="chartFooter"><span><img src="/icons/assets/pay3flow-mark.svg" width="15" height="15" alt="" />Pay3Flow</span><span>{say("Participant-only", "Только для участников")} · UTC</span></div>
<style>
.chartTools { display: flex; align-items: center; justify-content: space-between; gap: 8px; padding: 10px 17px; } .legend { display: flex; align-items: center; gap: 6px; color: var(--color-text-soft); font-size: 12px; } .buyDot, .sellDot { width: 6px; height: 6px; border-radius: 50%; background: var(--otc-buy); } .sellDot { margin-left: 8px; background: var(--otc-sell); } button { min-height: 36px; padding: 6px 8px; font-size: 11px; color: var(--color-text-soft); border-radius: 5px; } button.active { background: var(--color-panel); color: var(--color-text); } .chartLabel { padding: 0 17px; font-size: 12px; color: var(--color-text-soft); }
.chartPlot { position: relative; min-height: 330px; } svg { display: block; width: 100%; min-height: 330px; } .grid { stroke: var(--color-border); stroke-width: .7; } polyline { fill: none; stroke: var(--otc-buy); stroke-width: 2; } circle { fill: var(--otc-buy); } polyline.sell { stroke: var(--otc-sell); } circle.sell { fill: var(--otc-sell); } text { fill: var(--color-text-faint); font-size: 11px; } .empty { position: absolute; inset: 0; display: flex; flex-direction: column; justify-content: center; align-items: center; gap: 8px; padding: 24px; text-align: center; } .empty strong { background: var(--exchange-card-bg); font-size: 14px; padding: 5px 12px; } .empty p { max-width: 260px; background: var(--exchange-card-bg); color: var(--color-text-soft); font-size: 12px; line-height: 1.6; padding: 5px; } .chartFooter { display: flex; justify-content: space-between; gap: 12px; align-items: center; padding: 10px 17px 16px; color: var(--color-text-soft); font-size: 11px; } .chartFooter span { display: flex; align-items: center; gap: 5px; }
@media(max-width:640px) { .chartTools { flex-wrap: wrap; } }
</style>
