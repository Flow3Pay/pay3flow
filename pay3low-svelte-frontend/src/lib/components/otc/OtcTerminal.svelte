<script lang="ts">
  import { fade, slide } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { locale } from "$lib/i18n";
  import OtcPanel from "./OtcPanel.svelte";
  import OtcPanelToggle from "./OtcPanelToggle.svelte";
  export let baseAsset = "EVER", quoteAsset = "USDT", marketIcon = "/icons/assets/ever.svg", networkLabel = "Everscale ↔ Ethereum", venueLabel = "OTC";
  export let bridgeOpen = true, bookOpen = true, motionDuration = 320;
  export let selectedRate = "—", rateLabel = "", bookTitle = "", stats: { label:string; value:string; secondary?:boolean; unit?:string }[] = [];
  // Terminals whose book content brings its own card shell still get the generic panel here.
  export let bookPanel = true;
  $: say = (en:string,ru:string) => $locale === "ru" ? ru : en;
  function panelTransition(node:Element,options:{duration:number}) { return (window.matchMedia("(max-width:980px)").matches ? slide : fade)(node,{...options,easing:cubicOut}); }
</script>
<div class="marketStrip"><div class="marketSelector"><img src={marketIcon} width="38" height="38" alt="" /><div><div class="pairSelect"><strong>{baseAsset} / {quoteAsset}</strong></div><span>{networkLabel} <span class="marketTag">{venueLabel}</span></span></div></div><div class="marketStat last"><span>{rateLabel || say("Your selected rate","Выбранный курс")}</span><strong>{selectedRate} <small>{quoteAsset} / {baseAsset}</small></strong></div>{#each stats as stat}<div class="marketStat" class:secondaryStat={stat.secondary} class:volumeStat={!!stat.unit}><span>{stat.label}</span><strong>{stat.value}{#if stat.unit}{" "}<small>{stat.unit}</small>{/if}</strong></div>{/each}</div>
<slot name="status" />
<div class="terminalShell">
  <div class="otcGrid">
    <div class="terminalColumn chartColumn" id="otc-chart-column"><OtcPanel id="otc-chart" title={say("Chart","График")} collapsible={false}><slot name="chart" /></OtcPanel></div>
    <div class="seam bookSeam"><OtcPanelToggle title={bookTitle} controls="otc-book-column" bind:expanded={bookOpen} direction="right" /></div>
    {#if bookOpen}<div class="terminalColumn bookColumn" id="otc-book-column" transition:panelTransition={{duration:motionDuration}}>
      {#if bookPanel}<OtcPanel id="otc-book" title={bookTitle} collapsible={false}><slot name="book" /></OtcPanel>{:else}<slot name="book" />{/if}
    </div>{/if}
  </div>
  <div class="railColumn">
    {#if bridgeOpen}<div class="terminalColumn bridgeColumn" id="otc-bridge-column" transition:panelTransition={{duration:motionDuration}}><OtcPanel id="otc-bridge" title={say("Exchange","Обмен")} collapsible={false}><span slot="actions" class="panelMeta">{venueLabel}</span><slot name="ticket" /></OtcPanel></div>{/if}
    <div class="seam bridgeSeam"><OtcPanelToggle title={say("Exchange","Обмен")} controls="otc-bridge-column" bind:expanded={bridgeOpen} /></div>
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
  /* The desk chart and the orderbook stack in the main column; the ticket and the account rail sit beside them. */
  .terminalShell { display: grid; grid-template-columns: minmax(0, 1fr) minmax(300px, 380px); gap: 24px; align-items: start; }
  .otcGrid, .railColumn { display: flex; min-width: 0; flex-direction: column; }
  .terminalColumn { display: flex; width: 100%; min-width: 0; overflow: clip; }
  /* The gutter between the cards carries the collapse handle instead of costing an extra row. */
  .seam { display: flex; height: 24px; align-items: center; }
  .bookSeam { justify-content: flex-end; }
  .bridgeSeam { justify-content: flex-start; }
  .panelMeta { color: var(--color-text-faint); font-family: var(--font-mono); font-size: 12px; }
  @media (min-width: 981px) and (max-width: 1200px) { .marketStrip { grid-template-columns: minmax(220px, 1.3fr) repeat(3, minmax(0, 1fr)); } .secondaryStat { display: none; } }
  @media (max-width: 980px) {
    .terminalShell { display: flex; width: 100%; flex-direction: column; }
    .seam { height: 44px; }
    .bookSeam, .bridgeSeam { justify-content: center; }
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
