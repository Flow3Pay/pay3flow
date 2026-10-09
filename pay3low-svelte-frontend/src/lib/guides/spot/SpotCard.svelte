<script lang="ts">
  import { assetIcon, venueIcon } from "$lib/icons";
  import { locale, t } from "$lib/i18n";
  import type { RouteCandidate } from "$lib/exchange";
  import type { TutorialStep } from "$lib/route-tutorial";
  import { spotProfiles, spotTrade } from "./frames";
  import { ease, portion } from "../p2p/profile-motion";
  // svelte-ignore export_let_unused
  export let route: RouteCandidate;
  export let step: TutorialStep;
  export let frame = 0;
  export let playing = true;
  export let progress = 1;
  $: copy = (key: string, params: Record<string, string | number> = {}) => t(key, params, $locale);
  $: provider = step.provider.toLowerCase();
  $: profile = spotProfiles[provider]!;
  $: trade = spotTrade(step.pair, step.from, step.to);
  $: kind = step.frames[frame]?.kind;
  $: belowChart = profile.layout === "below-chart";
  $: travel = ease(portion(progress, 0, .8));
  $: formX = belowChart ? (trade?.side === "sell" ? 65 : 37) : 80;
  $: cursorX = kind === "open" ? 14 : kind === "verify" ? 17 + (formX - 17) * travel : formX;
  $: cursorY = kind === "open" ? 5 : kind === "verify" ? 18 + (belowChart ? 50 : 18) * travel : kind === "review" ? (belowChart ? 77 : 51) : kind === "act" ? 85 : 94;
</script>

{#snippet orderForm(side: "buy" | "sell", selected: boolean)}
  <div class="orderForm" class:selling={side === "sell"} class:inactive={!selected}>
    {#if !belowChart}
      <div class="formHeading">{copy(provider === "whitebird" ? "Trading Panel" : "Spot")}</div>
      <div class="sideTabs" class:highlight={selected && kind === "verify"}><span class:active={side === "buy"}>{copy("Buy")}</span><span class:active={side === "sell"}>{copy("Sell")}</span></div>
      <div class="orderTypes"><span>{copy("Limit")}</span><b>{copy("Market")}</b></div>
    {:else}
      <div class="formHeading" class:highlight={selected && kind === "verify"}>{copy(side === "buy" ? "Buy {asset}" : "Sell {asset}", { asset: trade?.base ?? step.to })}</div>
    {/if}
    <div class="available"><span>{copy("Available")}</span><span>— {side === "buy" ? trade?.quote : trade?.base}</span></div>
    <div class="orderField" class:highlight={selected && kind === "review"}>
      <small>{copy(side === "buy" ? profile.buyField : profile.sellField)}</small>
      {#if selected}<strong data-testid="spot-input">{step.amount ?? `— ${step.from}`}</strong>{:else}<strong>— {side === "buy" ? trade?.quote : trade?.base}</strong>{/if}
    </div>
    {#if !belowChart}<div class="amountSlider"><i></i><i></i><i></i><i></i><i></i></div>{/if}
    {#if selected && !belowChart}<div class="estimate"><small>{copy("Estimated receive")}</small><strong>{step.output ?? `— ${step.to}`}</strong></div>{/if}
    <div class="action" class:highlight={selected && kind === "act"}>{copy(side === "buy" ? "Buy {asset}" : "Sell {asset}", { asset: trade?.base ?? step.to })}<span>↗</span></div>
  </div>
{/snippet}

{#snippet orderbook()}
  <div class="orderbook"><div class="sectionLabel">{copy("Orderbook")}</div><div class="bookColumns"><span>{trade?.quote}</span><span>{trade?.base}</span></div>
    {#each [70, 48, 83, 38, 56] as width}<div class="bookRow"><i style={`width:${width}%`}></i><span>—</span><span>—</span></div>{/each}
    <div class="bookMid">⇄ <small>{copy("Market price")}</small></div>
    {#each [52, 76, 48, 84, 63] as width}<div class="bookRow bid"><i style={`width:${width}%`}></i><span>—</span><span>—</span></div>{/each}
  </div>
{/snippet}

{#snippet markets()}
  <div class="markets"><div class="sectionLabel">{copy("Markets")}</div><div class="marketSearch">⌕ <span>{trade?.base}</span></div>
    {#each [...new Set([trade?.base ?? step.from, "BTC", "ETH", "SOL"])] as asset}<div class="marketRow" class:highlight={asset === trade?.base && kind === "verify"}><img src={assetIcon(asset)} alt="" /><span>{asset}<small>/{trade?.quote}</small></span></div>{/each}
  </div>
{/snippet}

<div class="spotCard" class:binance={provider === "binance"} class:bybit={provider === "bybit"} class:mexc={provider === "mexc"} class:bitget={provider === "bitget"} class:whitebird={provider === "whitebird"} class:paused={!playing} data-testid={`${provider}-spot-card`} data-side={trade?.side} data-layout={profile.layout} data-frame-kind={kind} style={`--cursor-x:${cursorX}cqw;--cursor-y:${cursorY}cqh`}>
  <div class="venueHeader" class:highlight={kind === "open"}><img src={venueIcon(provider)} alt="" /><strong>{step.venue}</strong><span>Trade <b>Spot</b></span><span class="account">{copy(provider === "bybit" ? "Unified Trading Account" : provider === "whitebird" ? "Deposit / Transfer" : "Spot account")}</span></div>
  <div class="pairRow" class:highlight={kind === "verify"}><img src={assetIcon(trade?.base ?? step.from)} alt="" /><strong data-testid="spot-pair">{trade?.label ?? step.pair}</strong><span>⌄</span><small>{step.from} → {step.to}</small></div>
  <div class="terminal">
    {#if provider === "whitebird"}{@render markets()}{/if}
    {#if belowChart}{@render orderbook()}{/if}
    <div class="chartAndOrders">
      <div class="marketPreview"><div class="sectionLabel">{copy("Chart")} <span>15m · 1h · 4h</span></div>
        <svg viewBox="0 0 190 120" fill="none" preserveAspectRatio="none" aria-hidden="true">
          <path d="M0 28H190M0 59H190M0 90H190M36 0V120M78 0V120M120 0V120M162 0V120" stroke="currentColor" opacity=".08" />
          {#each [86, 78, 82, 72, 63, 70, 66, 58, 46, 50, 42, 30, 37, 24, 32, 19] as y, i}<path d={`M${i * 12 + 5} ${y - 12}V${y + 13}`} stroke={i % 3 === 2 ? "#f6465d" : "#26a17b"} /><path d={`M${i * 12 + 5} ${y - 3}V${y + 6}`} stroke={i % 3 === 2 ? "#f6465d" : "#26a17b"} stroke-width="5" /><path d={`M${i * 12 + 5} ${112 - (i % 4) * 3}V120`} stroke="#26a17b" stroke-width="6" opacity=".25" />{/each}
          <path d="M0 86 24 80 48 71 72 63 96 52 120 44 144 34 166 31 190 25" stroke="var(--accent)" opacity=".65" stroke-width="1" />
        </svg>
      </div>
      {#if belowChart}<div class="bottomOrders"><div class="bottomHeading">Spot <span>{copy("Limit")}</span><b>{copy("Market")}</b></div><div class="dualForms">{@render orderForm("buy", trade?.side === "buy")}{@render orderForm("sell", trade?.side === "sell")}</div></div>{/if}
    </div>
    {#if belowChart}{@render markets()}{:else}{@render orderbook()}{@render orderForm(trade?.side ?? "buy", true)}{/if}
  </div>
  <div class="historyRow" class:highlight={kind === "receive"}><span>{copy("Open Orders")}</span><strong>{copy(provider === "bitget" ? "Order History" : "Trade History")}</strong><span>{copy("Check actual fills")}</span></div>
  <div class="cursorTrack"><svg viewBox="0 0 32 38" fill="none"><path d="M3 2L27 22L15 24L9 35L3 2Z" fill="white" stroke="#151c25" stroke-width="2.5" stroke-linejoin="round" /></svg></div>
</div>
<style>
  .spotCard { --accent: #f0b90b; --surface: #20252d; --muted: #929aaa; --border: #2b313b; position: relative; container-type: size; box-sizing: border-box; overflow: hidden; width: calc(100% - 40px); max-width: 580px; height: 330px; margin: 44px auto 110px; border: 1px solid var(--border); border-radius: 12px; background: #161a20; color: #eaecef; box-shadow: 0 20px 60px #20304028; font-family: Inter, Arial, sans-serif; }
  .bybit { --accent: #f7a600; background: #101014; --surface: #252629; }.mexc { --accent: #2364ed; background: #111216; }.bitget { --accent: #00c8d3; background: #101112; }.whitebird { --accent: #3584ff; background: #13191f; }
  .venueHeader { display: flex; align-items: center; gap: 7px; height: 32px; padding: 0 12px; border-bottom: 1px solid var(--border); font-size: 11px; }.venueHeader img { width: 16px; height: 16px; }.venueHeader > span { display: flex; align-items: center; gap: 12px; font-size: 8px; color: var(--muted); margin-left: 8px; }.venueHeader .account { margin-left: auto; font-size: 6px; }.venueHeader b { color: var(--accent); border-bottom: 2px solid var(--accent); padding: 9px 0; }
  .pairRow { display: flex; align-items: center; gap: 6px; height: 32px; padding: 0 12px; border-bottom: 1px solid var(--border); }.pairRow img { width: 16px; height: 16px; border-radius: 50%; }.pairRow strong { font-size: 10px; }.pairRow > span { font-size: 10px; color: var(--muted); }.pairRow small { margin-left: auto; color: var(--muted); font-size: 7px; }
  .terminal { display: grid; grid-template-columns: minmax(0, 1fr) 15% 40%; height: calc(100% - 94px); }.binance .terminal { grid-template-columns: 16% minmax(0, 1fr) 17%; }.whitebird .terminal { grid-template-columns: 14% minmax(0, 1fr) 14% 40%; }.chartAndOrders { display: flex; flex-direction: column; min-width: 0; overflow: hidden; }.marketPreview { flex: 1; display: flex; flex-direction: column; padding: 10px 7px 5px; min-height: 0; }.sectionLabel { display: flex; justify-content: space-between; font-size: 7px; white-space: nowrap; overflow: hidden; }.sectionLabel span { font-size: 6px; color: var(--muted); }.marketPreview > svg { width: 100%; height: 100%; min-height: 0; margin-top: 9px; }
  .orderbook { border-inline: 1px solid var(--border); padding: 10px 5px; min-width: 0; overflow: hidden; }.bookColumns { display: flex; justify-content: space-between; margin: 12px 0 5px; color: var(--muted); font-size: 5px; }.bookRow { display: flex; justify-content: space-between; position: relative; font-size: 6px; color: #f6465d; height: 12px; align-items: center; }.bookRow i { position: absolute; height: 100%; right: 0; background: #f6465d22; }.bookRow span { position: relative; }.bookRow.bid { color: #26a17b; }.bookRow.bid i { background: #26a17b22; }.bookMid { display: flex; align-items: center; gap: 3px; margin: 8px 0; color: #26a17b; font-size: 10px; }.bookMid small { font-size: 5px; }
  .markets { border-left: 1px solid var(--border); min-width: 0; overflow: hidden; padding: 10px 5px; }.whitebird .markets { border-left: 0; border-right: 1px solid var(--border); }.marketSearch { display: flex; align-items: center; gap: 4px; background: var(--surface); padding: 6px 4px; margin: 9px 0; border-radius: 4px; font-size: 7px; color: var(--muted); }.marketRow { display: flex; align-items: center; gap: 3px; margin: 5px 0; padding: 4px 0; font-size: 6px; }.marketRow img { width: 10px; height: 10px; }.marketRow small { color: var(--muted); font-size: 5px; }
  .orderForm { display: flex; flex-direction: column; gap: 9px; min-width: 0; padding: 10px; }.formHeading { font-size: 8px; font-weight: 600; }.sideTabs { display: flex; border-radius: 5px; background: var(--surface); overflow: hidden; }.sideTabs span { flex: 1; text-align: center; font-size: 8px; padding: 6px; color: var(--muted); }.sideTabs span.active { background: #26a17b; color: #fff; }.selling .sideTabs span.active { background: #f6465d; }.orderTypes { display: flex; gap: 12px; font-size: 7px; color: var(--muted); }.orderTypes b { color: #eaecef; border-bottom: 2px solid var(--accent); padding-bottom: 4px; }.available { display: flex; justify-content: space-between; color: var(--muted); font-size: 6px; gap: 3px; }.orderField { border: 1px solid var(--border); border-radius: 5px; padding: 8px; background: var(--surface); display: grid; gap: 6px; min-width: 0; }.orderField small, .estimate small { font-size: 6px; color: var(--muted); }.orderField strong { font-size: 10px; overflow-wrap: anywhere; font-variant-numeric: tabular-nums; }.estimate { display: grid; gap: 5px; }.estimate strong { font-size: 8px; font-variant-numeric: tabular-nums; overflow-wrap: anywhere; }.amountSlider { display: flex; align-items: center; justify-content: space-between; height: 3px; background: var(--border); margin: 2px 0; }.amountSlider i { width: 4px; height: 4px; background: var(--muted); transform: rotate(45deg); }.action { display: flex; justify-content: center; align-items: center; gap: 6px; min-height: 26px; margin-top: auto; border-radius: 5px; background: #26a17b; color: #fff; font-size: 9px; font-weight: 600; }.selling .action { background: #f6465d; }
  .bottomOrders { border-top: 1px solid var(--border); }.bottomHeading { display: flex; align-items: center; gap: 10px; padding: 6px 8px 0; font-size: 7px; }.bottomHeading span { margin-left: auto; color: var(--muted); }.bottomHeading b { color: var(--accent); }.dualForms { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); }.dualForms .orderForm { padding: 7px 8px 9px; gap: 5px; }.dualForms .orderField { padding: 6px; gap: 4px; }.dualForms .action { min-height: 23px; font-size: 8px; }.inactive { opacity: .5; }
  .historyRow { display: flex; align-items: center; gap: 12px; height: 28px; padding: 0 12px; border-top: 1px solid var(--border); font-size: 7px; color: var(--muted); }.historyRow strong { color: #eaecef; border-bottom: 2px solid var(--accent); padding: 7px 0; }.historyRow > span:last-child { margin-left: auto; font-size: 6px; }
  .highlight { box-shadow: inset 0 0 0 2px var(--accent); }.cursorTrack { position: absolute; z-index: 2; left: 0; top: 0; width: 22px; transform: translate3d(var(--cursor-x), var(--cursor-y), 0); transition: transform .2s linear; pointer-events: none; }.cursorTrack svg { width: 22px; height: 27px; filter: drop-shadow(0 2px 2px #0004); }
  @container (max-width: 350px) { .orderForm { gap: 7px; padding: 8px; }.orderField { padding: 6px; }.orderField strong { font-size: 9px; }.venueHeader .account { display: none; }.sectionLabel span { display: none; }.marketRow small { display: none; }.historyRow { gap: 9px; }.dualForms .orderForm { padding-inline: 6px; }.dualForms .orderField strong { font-size: 8px; } }
  @media (max-height: 820px) and (min-width: 761px) { .spotCard { margin-top: 26px; }.orderForm { gap: 6px; }.orderField { padding: 6px; }.binance .available { display: none; } }
  @media (max-width: 760px) { .spotCard { width: calc(100% - 28px); margin-top: 24px; }.historyRow > span:last-child { display: none; }.binance .available { display: none; } }
  @media (prefers-reduced-motion: reduce) { .cursorTrack { transition: none; } }
</style>
