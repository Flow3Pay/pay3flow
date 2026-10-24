<script lang="ts">
  import { locale } from "$lib/i18n";
  import { completedVolume } from "$lib/otc/terminal";
  import { percentChange, quoteObservations } from "$lib/otc/market";
  import { decimal, rate, type Config, type Direction, type Proposal, type Session, type Trade } from "$lib/otc/service";
  export let config: Config | null = null;
  export let session: Session | null = null;
  export let listings: Proposal[] = [];
  export let activeTrades: Trade[] = [];
  export let trades: Trade[] = [];
  export let selected: Trade | null = null;
  export let direction: Direction = "buy";
  $: say = (en: string, ru: string) => $locale === "ru" ? ru : en;
  $: observations = quoteObservations(trades);
  $: latest = observations.at(-1) ?? null;
  $: change = percentChange(observations);
  $: shownRate = selected && selected.terms.direction === direction ? rate(selected.terms) : latest?.price ?? null;
  $: shownRoute = direction === "buy" ? "USDT → EVER" : "EVER → USDT";
  $: volume = completedVolume(trades);
  $: deskOnline = !!config?.available;
</script>

<div class="marketStrip">
  <div class="marketSelector">
    <img src="/icons/assets/ever.svg" width="40" height="40" alt="" />
    <div class="marketIdentity">
      <div class="pairSelect"><strong>EVER / USDT</strong><span class="otcTag">OTC</span></div>
      <span class="marketRoute">Everscale ↔ Ethereum</span>
      <span class="routeHint">{shownRoute} · {say("direct desk", "прямой деск")}</span>
    </div>
  </div>
  <div class="marketStat priceStat">
    <span>{say("Your offer price", "Цена вашего оффера")}</span>
    <strong>{shownRate ?? "—"}{#if change !== null && !selected}<em class:negative={change < 0}>{change >= 0 ? "+" : ""}{change.toFixed(2)}%</em>{/if}</strong>
    <small>USDT / EVER</small>
  </div>
  <div class="marketStat deskStat">
    <span>{say("Desk", "Деск")}</span>
    <strong><i class:offline={!deskOnline}></i>{config?.desk_name || "—"}</strong>
    <small>{deskOnline ? say("OTC · direct", "OTC · напрямую") : say("Trading paused", "Торги приостановлены")}</small>
  </div>
  <div class="marketStat secondaryStat">
    <span>{say("Open offers", "Открытые офферы")}</span>
    <strong>{listings.length}</strong>
    <small>{say("public desk Proposals", "публичные предложения")}</small>
  </div>
  <div class="marketStat">
    <span>{say("Your open orders", "Ваши открытые ордера")}</span>
    <strong>{session ? activeTrades.length : "—"}</strong>
    <small>{session ? say("in progress", "в работе") : say("connect wallet", "подключите кошелёк")}</small>
  </div>
  <div class="marketStat volumeStat">
    <span>{say("Your completed volume", "Ваш завершённый объём")}</span>
    <strong>{session ? decimal(volume, 6) : "—"} <small>USDT</small></strong>
    <small>{say("settled with this desk", "расчёты с этим деском")}</small>
  </div>
</div>

<style>
  .marketStrip { display: grid; grid-template-columns: minmax(190px, 1.1fr) repeat(5, minmax(0, 1fr)); align-items: stretch; gap: 0; min-height: 92px; padding: 0 4px; margin-bottom: 22px; border: 1px solid var(--color-border); border-radius: 10px; background: var(--color-paper); }
  .marketSelector { display: flex; align-items: center; gap: 11px; min-width: 0; padding: 15px 18px; }
  .marketSelector > img { flex: 0 0 auto; border-radius: 50%; }
  .marketIdentity { display: grid; gap: 3px; min-width: 0; }
  .pairSelect { display: flex; align-items: center; gap: 6px; }
  .pairSelect strong { font-size: 14px; font-weight: 800; letter-spacing: -.02em; }
  .otcTag { padding: 2px 4px; border-radius: 3px; background: var(--color-panel); color: var(--color-text-soft); font-family: var(--font-mono); font-size: 9px; }
  .marketRoute { color: var(--color-text-soft); font-size: 11px; }
  .routeHint { color: var(--color-text-faint); font-family: var(--font-mono); font-size: 9px; }
  .marketStat { display: grid; align-content: center; gap: 5px; min-width: 0; padding: 14px 15px; border-left: 1px solid var(--color-border); }
  .marketStat > span { color: var(--color-text-soft); font-size: 10px; }
  .marketStat strong { display: flex; align-items: baseline; gap: 6px; overflow-wrap: anywhere; font-family: var(--font-mono); font-size: 13px; font-weight: 400; }
  .marketStat strong em { font-style: normal; font-size: 10px; color: var(--otc-buy); }
  .marketStat strong em.negative { color: var(--otc-sell); }
  .marketStat strong > small { font-size: 9px; color: var(--color-text-soft); }
  .marketStat > small { color: var(--color-text-faint); font-size: 9px; white-space: nowrap; }
  .marketStat.priceStat strong { font-size: 20px; letter-spacing: -.04em; }
  .deskStat strong { display: flex; align-items: center; gap: 6px; font-family: var(--font-sans); font-size: 11px; font-weight: 650; }
  .deskStat i { width: 6px; height: 6px; flex: 0 0 auto; border-radius: 50%; background: var(--otc-buy); box-shadow: 0 0 0 3px var(--otc-buy-soft); }
  .deskStat i.offline { background: var(--color-text-faint); box-shadow: none; }
  @media (max-width: 1200px) and (min-width: 981px) { .marketStrip { grid-template-columns: minmax(180px, 1.2fr) repeat(4, minmax(0, 1fr)); } .secondaryStat { display: none; } }
  @media (max-width: 980px) { .marketStrip { grid-template-columns: repeat(3, minmax(0, 1fr)); } .marketSelector { grid-column: 1 / -1; border-bottom: 1px solid var(--color-border); } .marketStat { border-left: 0; border-right: 1px solid var(--color-border); } .marketStat:nth-child(4), .marketStat:nth-child(5) { border-right: 0; } }
  @media (max-width: 640px) { .marketStrip { grid-template-columns: repeat(2, minmax(0, 1fr)); } .secondaryStat { display: none; } .marketStat.priceStat, .marketStat.deskStat { border-right: 0; } .marketStat { padding: 12px 13px; } .marketSelector { padding: 13px; } .marketStat.priceStat strong { font-size: 17px; } }
</style>
