<script lang="ts">
  import { locale } from "$lib/i18n";
  import OtcPanel from "./OtcPanel.svelte";
  export let baseAsset = "EVER", quoteAsset = "USDT", marketIcon = "/icons/assets/ever.svg", networkLabel = "Everscale ↔ Ethereum", venueLabel = "OTC";
  export let selectedRate = "—", rateLabel = "", bookTitle = "", stats: { label:string; value:string; secondary?:boolean; unit?:string }[] = [];
  // Terminals whose content brings its own card shell still get the generic panel here.
  export let bookPanel = true, chartPanel = true;
  $: say = (en:string,ru:string) => $locale === "ru" ? ru : en;
</script>
<div class="marketStrip"><div class="marketSelector"><img src={marketIcon} width="38" height="38" alt="" /><div><div class="pairSelect"><strong>{baseAsset} / {quoteAsset}</strong></div><span>{networkLabel} <span class="marketTag">{venueLabel}</span></span></div></div><div class="marketStat last"><span>{rateLabel || say("Your selected rate","Выбранный курс")}</span><strong>{selectedRate} <small>{quoteAsset} / {baseAsset}</small></strong></div>{#each stats as stat}<div class="marketStat" class:secondaryStat={stat.secondary} class:volumeStat={!!stat.unit}><span>{stat.label}</span><strong>{stat.value}{#if stat.unit}{" "}<small>{stat.unit}</small>{/if}</strong></div>{/each}</div>
<slot name="status" />
<div class="terminalShell">
  <div class="otcGrid">
    <div class="terminalColumn chartColumn" id="otc-chart-column">
      {#if chartPanel}<OtcPanel id="otc-chart" title={say("Chart","График")} collapsible={false}><slot name="chart" /></OtcPanel>{:else}<slot name="chart" />{/if}
    </div>
    <div class="terminalColumn bookColumn" id="otc-book-column">
      {#if bookPanel}<OtcPanel id="otc-book" title={bookTitle} collapsible={false}><slot name="book" /></OtcPanel>{:else}<slot name="book" />{/if}
    </div>
  </div>
  <div class="railColumn">
    <div class="terminalColumn bridgeColumn" id="otc-bridge-column"><OtcPanel id="otc-bridge" title={say("Exchange","Обмен")} collapsible={false}><slot name="ticket" /></OtcPanel></div>
    <slot name="rail" />
  </div>
</div>
<style>
  .marketStrip { display: grid; grid-template-columns: minmax(220px, 1.25fr) repeat(5, minmax(0, 1fr)); gap: 20px; align-items: center; min-height: 96px; margin-bottom: 24px; padding: 20px; border: 1px solid var(--color-border); border-radius: var(--otc-card-radius, 24px); background: var(--color-paper); }
  .marketSelector { display: flex; min-width: 0; align-items: center; gap: 12px; padding-right: 20px; border-right: 1px solid var(--color-border); }
  .marketSelector > img { flex: 0 0 38px; border-radius: 50%; }
  .marketSelector > div { min-width: 0; }
  .pairSelect { font-size: 14px; font-weight: 650; white-space: nowrap; }
  .marketSelector > div > span { display: flex; flex-wrap: wrap; align-items: center; gap: 4px 8px; margin-top: 4px; color: var(--color-text-soft); font-size: 12px; }
  .marketTag { padding: 2px 4px; border-radius: 4px; background: var(--color-panel); font-family: var(--font-mono); font-size: 11px; }
  .marketStat { display: grid; min-width: 0; align-content: start; gap: 6px; }
  .marketStat > span { color: var(--color-text-soft); font-size: 12px; line-height: 1.5; }
  .marketStat strong { overflow-wrap: anywhere; font-family: var(--font-mono); font-weight: 400; font-size: 13px; font-variant-numeric: tabular-nums; }
  .marketStat strong small { color: var(--color-text-soft); font-size: 11px; }
  .marketStat.last strong { font-size: 18px; letter-spacing: -.04em; }
  /* The chart and the orderbook stack in the main column; the ticket sits beside them. */
  .terminalShell { display: grid; grid-template-columns: minmax(0, 1fr) minmax(300px, 380px); gap: 24px; align-items: start; }
  .otcGrid, .railColumn { display: flex; min-width: 0; flex-direction: column; gap: 24px; }
  .terminalColumn { display: flex; width: 100%; min-width: 0; overflow: clip; }
  .panelMeta { color: var(--color-text-faint); font-family: var(--font-mono); font-size: 12px; }
  @media (min-width: 981px) and (max-width: 1200px) { .marketStrip { grid-template-columns: minmax(220px, 1.3fr) repeat(3, minmax(0, 1fr)); } .secondaryStat { display: none; } }
  @media (max-width: 980px) {
    .terminalShell { display: flex; width: 100%; flex-direction: column; gap: 20px; }
    .marketStrip { grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 16px; }
    .marketSelector { grid-column: span 2; border-right: 0; padding-right: 0; }
    .marketStat:not(.last) { border-top: 1px solid var(--color-border); padding-top: 12px; }
  }
  @media (max-width: 640px) {
    .marketStrip { grid-template-columns: repeat(2, minmax(0, 1fr)); padding: 16px; gap: 16px; }
    .marketSelector { grid-column: 1 / -1; padding-bottom: 16px; border-bottom: 1px solid var(--color-border); }
    .marketStat:not(.last) { padding-top: 0; border-top: 0; }
    .secondaryStat { display: none; }
    .volumeStat { grid-column: 1 / -1; padding-top: 12px !important; border-top: 1px solid var(--color-border) !important; }
    .marketStat.last strong { font-size: 16px; }
  }
</style>
