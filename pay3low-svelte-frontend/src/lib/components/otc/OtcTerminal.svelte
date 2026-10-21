<script lang="ts">
  import { fade, slide } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { locale } from "$lib/i18n";
  import OtcPanel from "./OtcPanel.svelte";
  import OtcPanelToggle from "./OtcPanelToggle.svelte";
  export let bridgeOpen = true, bookOpen = true, motionDuration = 320;
  export let selectedRate = "—", rateLabel = "", bookTitle = "", stats: { label:string; value:string; secondary?:boolean; unit?:string }[] = [];
  $: say = (en:string,ru:string) => $locale === "ru" ? ru : en;
  function panelTransition(node:Element,options:{duration:number}) { return (window.matchMedia("(max-width:980px)").matches ? slide : fade)(node,{...options,easing:cubicOut}); }
</script>
<div class="marketStrip"><div class="marketSelector"><img src="/icons/assets/ever.svg" width="38" height="38" alt="" /><div><div class="pairSelect"><strong>EVER / USDT</strong></div><span>Everscale ↔ Ethereum <span class="marketTag">OTC</span></span></div></div><div class="marketStat last"><span>{rateLabel || say("Your selected rate","Выбранный курс")}</span><strong>{selectedRate} <small>USDT / EVER</small></strong></div>{#each stats as stat}<div class="marketStat" class:secondaryStat={stat.secondary} class:volumeStat={!!stat.unit}><span>{stat.label}</span><strong>{stat.value}{#if stat.unit}{" "}<small>{stat.unit}</small>{/if}</strong></div>{/each}</div>
<slot name="status" />
<div class="otcGrid" class:bridgeClosed={!bridgeOpen} class:bookClosed={!bookOpen}>
  {#if bridgeOpen}<div class="bridgeColumn" id="otc-bridge-column" transition:panelTransition={{duration:motionDuration}}><OtcPanel id="otc-bridge" title={say("Exchange","Обмен")} collapsible={false}><span slot="actions" class="panelMeta">OTC</span><slot name="ticket" /></OtcPanel></div>{/if}
  <div class="bridgeToggle"><OtcPanelToggle title={say("Exchange","Обмен")} controls="otc-bridge-column" bind:expanded={bridgeOpen} /></div>
  <div class="chartColumn" id="otc-chart-column"><OtcPanel id="otc-chart" title={say("Chart","График")} collapsible={false}><slot name="chart" /></OtcPanel></div>
  <div class="bookToggle"><OtcPanelToggle title={bookTitle} controls="otc-book-column" bind:expanded={bookOpen} direction="right" /></div>
  {#if bookOpen}<div class="bookColumn" id="otc-book-column" transition:panelTransition={{duration:motionDuration}}><OtcPanel id="otc-book" title={bookTitle} collapsible={false}><slot name="book" /></OtcPanel></div>{/if}
</div>
<style>
  .marketStrip { display: grid; grid-template-columns: minmax(220px, 1.25fr) repeat(5, minmax(0, 1fr)); gap: 20px; align-items: center; min-height: 96px; margin-bottom: 24px; padding: 20px; border: 1px solid var(--color-border); border-radius: 12px; background: var(--color-paper); }
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
  .otcGrid { display: grid; grid-template-columns: minmax(0, 1fr) 32px minmax(0, 1.3fr) 32px minmax(0, .85fr); align-items: stretch; transition: grid-template-columns .32s cubic-bezier(.22, 1, .36, 1); }
  .bridgeColumn { grid-column: 1; grid-row: 1; }
  .chartColumn { grid-column: 3; grid-row: 1; }
  .bookColumn { grid-column: 5; grid-row: 1; }
  .bridgeColumn, .chartColumn, .bookColumn { display: flex; min-width: 0; }
  .bridgeColumn, .bookColumn { overflow: clip; }
  .bridgeToggle, .bookToggle { display: grid; align-items: center; justify-items: center; grid-row: 1; }
  .bridgeToggle { grid-column: 2; }
  .bookToggle { grid-column: 4; }
  .otcGrid.bridgeClosed { grid-template-columns: minmax(0, 0fr) 32px minmax(0, 1.3fr) 32px minmax(0, .85fr); }
  @media (min-width: 981px) {
    .otcGrid.bookClosed .chartColumn { grid-column: 3 / -1; }
    .otcGrid.bookClosed .bookToggle { grid-column: 5; align-self: start; justify-self: end; z-index: 1; height: 56px; margin-right: 8px; }
  }

  .panelMeta { color:var(--color-text-faint); font-family:var(--font-mono); font-size:12px; }
  @media (min-width:981px) and (max-width:1200px) { .marketStrip { grid-template-columns:minmax(220px,1.3fr) repeat(3,minmax(0,1fr)); } .secondaryStat { display:none; } }
  @media (max-width:980px) {
    .otcGrid,.otcGrid.bridgeClosed,.otcGrid.bookClosed,.otcGrid.bridgeClosed.bookClosed { display:flex; width:100%; flex-direction:column; }
    .bridgeColumn,.chartColumn,.bookColumn { width:100%; }
    .bridgeToggle,.bookToggle { width:100%; height:44px; }
    .marketStrip { grid-template-columns:repeat(3,minmax(0,1fr)); gap:16px; }
    .marketSelector { grid-column:span 2; border-right:0; padding-right:0; }
    .marketStat:not(.last) { border-top:1px solid var(--color-border); padding-top:12px; }
  }
  @media (max-width:640px) {
    .marketStrip { grid-template-columns:repeat(2,minmax(0,1fr)); padding:16px; gap:16px; }
    .marketSelector { grid-column:1/-1; padding-bottom:16px; border-bottom:1px solid var(--color-border); }
    .marketStat:not(.last) { padding-top:0; border-top:0; }
    .secondaryStat { display:none; }
    .volumeStat { grid-column:1/-1; padding-top:12px!important; border-top:1px solid var(--color-border)!important; }
    .marketStat.last strong { font-size:16px; }
  }
  @media (prefers-reduced-motion:reduce) { .otcGrid { transition:none; } }
</style>
