<script lang="ts">
  import { assetIcon, venueIcon } from "$lib/icons";
  import { locale, t } from "$lib/i18n";
  import type { RouteCandidate } from "$lib/exchange";
  import type { TutorialStep } from "$lib/route-tutorial";
  import { spotMarket, spotProfiles } from "./frames";
  import { cursorMotion } from "./cursor-motion";

  export let route: RouteCandidate;
  export let step: TutorialStep | undefined;
  export let frame = 0;
  export let playing = true;
  export let progress = 1;
  $: copy = (key: string, params: Record<string, string | number> = {}) => t(key, params, $locale);
  $: provider = step?.provider.toLowerCase() ?? "";
  $: market = step && spotMarket(step);
  $: base = market?.base ?? step?.from ?? "";
  $: quote = market?.quote ?? step?.to ?? "";
  $: buying = market?.buying;
  $: kind = step?.frames[frame]?.kind;
  $: accent = ({ binance: "#f0b90b", bybit: "#f7a600", mexc: "#357aff", whitebird: "#468bff", bitget: "#00d4d4", okx: "#ffffff" } as Record<string, string>)[provider] ?? "#357aff";
  $: input = step?.amount ?? (step?.from === route.source_currency ? `${route.source_amount ?? (route.source_amount_minor == null ? "" : route.source_amount_minor / 100)} ${step.from}` : undefined);
  $: amount = (input?.endsWith(` ${step?.from}`) ? input.slice(0, -(step!.from.length + 1)) : input) || "—";
  $: action = market ? copy(buying ? "Buy {asset}" : "Sell {asset}", { asset: base }) : copy("Check the exchange");
  // Deterministic illustration data; never presented as a live quote or real holdings.
  const candles = Array.from({ length: 48 }, (_, i) => {
    const open = 140 - i * 1.1 + Math.sin(i * .62) * 22;
    const close = open + Math.sin(i * 1.73 + .4) * 14;
    return { x: 12 + i * 7.9, open, close, high: Math.min(open, close) - 5 - i % 7, low: Math.max(open, close) + 7 + i % 5, volume: 7 + (i * 13) % 28 };
  });
  const depth = [64, 38, 79, 46, 92, 57];
</script>

<div class="terminal" class:paused={!playing} class:whitebird={provider === "whitebird"} class:binance={provider === "binance"} class:mexc={provider === "mexc"} class:bitget={provider === "bitget"} data-testid="spot-terminal" data-provider={provider} data-frame-kind={kind} data-order-side={market ? buying ? "buy" : "sell" : "unknown"} style={`--venue-accent:${accent}`}>
  <div class="terminalHeader">
    <div class="brand"><img src={venueIcon(provider)} alt="" /><strong>{step?.venue}</strong></div>
    <span class="navSelected">{copy("Spot")}</span><span class="navItem">{copy("Markets")}</span>
    <span class="demoLabel"><i></i>{copy("Trading demonstration")}</span>
  </div>
  <div class="screen">
    <div class="marketHeader" class:focus={kind === "open"} data-testid="spot-market-pair">
      <div class="pair"><img src={assetIcon(base)} alt="" /><strong>{base}<span>/{quote}</span></strong><span class="chevron">⌄</span></div>
      <div class="quote"><strong>—</strong><small>{copy("Current price on the platform")}</small></div>
      <div class="stat"><small>{copy("24h high")}</small><span>—</span></div><div class="stat"><small>{copy("24h low")}</small><span>—</span></div><div class="stat"><small>{copy("24h volume")}</small><span>—</span></div>
    </div>
    <div class="workspace">
      {#if provider === "whitebird" || provider === "binance"}
        <div class="watchlist"><div class="panelTitle">{copy("Markets")}</div><div class="search">⌕ <span>{copy("Search")}</span></div>{#each Array.from(new Set([base, "ETH", "SOL", "TRX", "USDC"])) as asset, i}<div class:chosen={i === 0}><img src={assetIcon(asset)} alt="" /><span>{asset}<small>/USDT</small></span></div>{/each}</div>
      {/if}
      <div class="chartPanel">
        <div class="chartTabs"><strong>{copy("Chart")}</strong><span>{copy("Trading info")}</span><span class="expand">⛶</span></div>
        <div class="intervals"><span>15m</span><b>1h</b><span>4h</span><span>1D</span><i></i><span>TradingView</span></div>
        <div class="chart">
          <span class="chartPair">{base}/{quote} · 1h</span>
          <svg class="candles" viewBox="0 0 410 245" preserveAspectRatio="none" fill="none">
            {#each [45, 90, 135, 180] as y}<path d={`M0 ${y}H410`} stroke="#20262f" stroke-dasharray="2 5" />{/each}
            {#each [70, 140, 210, 280, 350] as x}<path d={`M${x} 0V245`} stroke="#20262f" stroke-dasharray="2 5" />{/each}
            {#each candles as candle}<g class:up={candle.close < candle.open} class:down={candle.close >= candle.open}><path d={`M${candle.x} ${candle.high}V${candle.low}`} /><rect x={candle.x - 2.4} y={Math.min(candle.open, candle.close)} width="4.8" height={Math.max(1.5, Math.abs(candle.open - candle.close))} /><rect class="volume" x={candle.x - 2.4} y={245 - candle.volume} width="4.8" height={candle.volume} /></g>{/each}
            <path d="M0 101H410" stroke="#36cba0" stroke-dasharray="3 4" opacity=".45" />
          </svg>
          <div class="chartTimes"><span>08:00</span><span>12:00</span><span>16:00</span><span>20:00</span></div>
          <span class="chartDisclaimer">{copy("Illustrative chart")}</span>
        </div>
      </div>
      <div class="orderBook"><div class="panelTitle">{copy("Order book")}<span>≡</span></div><div class="bookLabels"><span>{copy("Price")}</span><span>{copy("Amount")}</span></div>{#each depth as width}<div class="depthRow ask" style={`--depth:${width}%`}><span>—</span><span>—</span></div>{/each}<div class="spread">— <span>{quote}</span></div>{#each depth.slice().reverse() as width}<div class="depthRow bid" style={`--depth:${width}%`}><span>—</span><span>—</span></div>{/each}</div>
      <div class="orderPanel" data-testid="spot-order-form">
        <div class="panelTitle">{copy("Spot")}<span>⚙</span></div>
        <div class="sideTabs" class:focus={kind === "verify"}><span class:selected={buying === true} class="buy">{copy("Buy")}</span><span class:selected={buying === false} class="sell">{copy("Sell")}</span></div>
        <div class="orderTypes" class:focus={kind === "review"}><span>{copy("Limit")}</span><strong>{copy("Market")}</strong><span>TP/SL</span></div>
        <div class="balance"><span>{copy("Available")}</span><b>— {step?.from}</b></div>
        <div class="priceField"><span>{copy("Price")}</span><strong>{copy("Market price")}</strong></div>
        <div class="amountField" class:focus={kind === "review"} data-testid="spot-input-amount"><span>{copy((buying ? spotProfiles[provider]?.buyField : spotProfiles[provider]?.sellField) ?? (buying ? "Total" : "Amount"))}</span><div><strong>{kind === "open" || kind === "verify" ? "—" : amount}</strong><b>{step?.from}</b></div></div>
        <div class="amountSlider"><i></i><i></i><i></i><i></i><i></i></div>
        <div class="estimateLine"><span>{copy("You receive")}</span><b>{step?.output ?? `— ${step?.to}`}</b></div>
        <div class="feeLine"><span>{copy("Trading fee")}</span><span>{copy("Check on the platform")}</span></div>
        <div class="tradeAction" class:sell={buying === false} class:focus={kind === "act"} class:pressed={kind === "act" && progress > .75} data-testid="spot-order-action">{action}<svg viewBox="0 0 24 24" fill="none"><path d="M5 12h14m-5-5 5 5-5 5" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" /></svg></div>
      </div>
    </div>
    <div class="history" class:focus={kind === "receive"} data-testid="spot-order-history"><div class="historyTabs"><span class:active={kind !== "receive"}>{copy("Open orders")}</span><span class:active={kind === "receive"}>{copy("Order history")}</span><span>{copy("Assets")}</span></div><div class="historyHint"><span>{kind === "receive" ? "◎" : "≡"}</span>{copy(kind === "receive" ? "Check the filled amount and your balance on the platform" : "Your orders appear here on the platform")}</div></div>
    <svg class="cursor" use:cursorMotion={{ kind, progress }} viewBox="0 0 28 34" fill="none"><path d="M3 2L23 19L13 21L8 30L3 2Z" fill="white" stroke="#0b0e11" stroke-width="2" stroke-linejoin="round" /></svg>
  </div>
  <div class="caption"><span class="captionNumber">{String(frame + 1).padStart(2, "0")}</span><div><strong>{step?.frames[frame]?.title}</strong><small>{copy("Illustration · the platform interface may differ")}</small></div><span class="captionPair">{step?.from} → {step?.to}</span></div>
</div>

<style>
  .terminal { --line: #252a32; --muted: #8c96a5; --green: #2dc59a; --red: #ef6676; background: #0b0e11; color: #e8edf4; font-family: Inter, Arial, sans-serif; font-size: 11px; line-height: 1.4; }
  .terminalHeader { display: flex; align-items: center; gap: 22px; height: 48px; padding: 0 20px; border-bottom: 1px solid var(--line); background: #101318; }.brand { display: flex; align-items: center; gap: 8px; }.brand img { width: 23px; height: 23px; object-fit: contain; border-radius: 4px; }.brand strong { font-size: 15px; letter-spacing: -.03em; }.navSelected { color: var(--venue-accent); }.navItem { color: var(--muted); }.demoLabel { margin-left: auto; display: flex; align-items: center; gap: 6px; color: var(--muted); font-size: 10px; }.demoLabel i { width: 5px; height: 5px; background: var(--venue-accent); border-radius: 50%; }
  .screen { position: relative; }.marketHeader { display: flex; align-items: center; gap: 28px; min-height: 62px; padding: 12px 20px; border-bottom: 1px solid var(--line); box-sizing: border-box; }.pair { display: flex; align-items: center; gap: 7px; flex-shrink: 0; }.pair img { width: 24px; height: 24px; }.pair strong { font-size: 15px; }.pair strong span { color: var(--muted); font-weight: 500; }.chevron { color: var(--muted); }.quote { display: grid; gap: 2px; }.quote strong { font-size: 16px; color: var(--green); }.quote small, .stat small { font-size: 8px; color: var(--muted); }.stat { display: grid; gap: 5px; }.stat span { font-size: 10px; }
  .workspace { display: grid; grid-template-columns: minmax(0, 1fr) 145px 235px; min-height: 312px; }.chartPanel { min-width: 0; border-right: 1px solid var(--line); }.chartTabs { display: flex; align-items: center; gap: 18px; height: 34px; padding: 0 14px; border-bottom: 1px solid var(--line); font-size: 10px; }.chartTabs strong { align-self: stretch; display: flex; align-items: center; border-bottom: 2px solid var(--venue-accent); }.chartTabs > span { color: var(--muted); }.chartTabs .expand { margin-left: auto; font-size: 16px; }.intervals { display: flex; align-items: center; gap: 14px; height: 28px; padding: 0 14px; color: var(--muted); font-size: 8px; }.intervals b { color: var(--venue-accent); }.intervals i { flex: 1; }.intervals > span:last-child { font-size: 7px; }.chart { position: relative; height: 246px; padding: 0 12px; overflow: hidden; }.chartPair { position: absolute; top: 8px; left: 18px; color: var(--muted); font-size: 9px; }.candles { display: block; width: 100%; height: 214px; }.up { stroke: var(--green); fill: var(--green); }.down { stroke: var(--red); fill: var(--red); }.volume { opacity: .22; stroke: none; }.chartTimes { display: flex; justify-content: space-around; font-size: 7px; color: var(--muted); }.chartDisclaimer { position: absolute; bottom: 6px; left: 18px; color: var(--muted); font-size: 7px; }
  .panelTitle { display: flex; justify-content: space-between; align-items: center; height: 34px; padding: 0 12px; border-bottom: 1px solid var(--line); font-size: 10px; font-weight: 600; }.panelTitle > span { color: var(--muted); }.orderBook { border-right: 1px solid var(--line); }.bookLabels { display: flex; justify-content: space-between; padding: 9px 12px 5px; color: var(--muted); font-size: 7px; }.depthRow { display: flex; justify-content: space-between; position: relative; margin: 0 5px; padding: 2px 7px; font: 9px var(--font-mono, monospace); height: 15px; box-sizing: border-box; }.depthRow::before { content: ''; position: absolute; width: var(--depth); top: 0; bottom: 0; right: 0; background: currentColor; opacity: .09; }.ask { color: var(--red); }.bid { color: var(--green); }.depthRow > span:last-child { color: #bac3d0; }.spread { padding: 9px 12px; color: var(--green); font-size: 16px; }.spread > span { font-size: 9px; color: var(--muted); }
  .orderPanel { min-width: 0; padding: 0 12px 12px; background: #101419; }.orderPanel > .panelTitle { padding: 0; }.sideTabs { display: flex; margin-top: 10px; padding: 3px; border-radius: 6px; background: #20262d; }.sideTabs > span { flex: 1; text-align: center; padding: 5px; color: var(--muted); font-size: 10px; font-weight: 600; border-radius: 4px; }.sideTabs .selected.buy { color: #081b16; background: var(--green); }.sideTabs .selected.sell { color: #230c10; background: var(--red); }.orderTypes { display: flex; align-items: center; gap: 16px; margin-top: 8px; padding: 4px 0; font-size: 9px; color: var(--muted); }.orderTypes strong { padding-bottom: 3px; color: var(--venue-accent); border-bottom: 2px solid var(--venue-accent); }.balance, .estimateLine, .feeLine { display: flex; justify-content: space-between; flex-wrap: wrap; gap: 4px; font-size: 8px; color: var(--muted); }.balance { margin: 8px 0; }.balance b, .estimateLine b { color: #d2dae6; font-weight: 500; overflow-wrap: anywhere; }.priceField { display: flex; justify-content: space-between; padding: 9px; background: #1b2028; border: 1px solid #2b323d; border-radius: 5px; font-size: 8px; color: var(--muted); }.priceField strong { font-weight: 500; color: #bcc4ce; }.amountField { margin-top: 8px; padding: 8px 9px; border: 1px solid #3d4654; border-radius: 5px; background: #151a21; }.amountField > span { font-size: 8px; color: var(--muted); }.amountField > div { display: flex; justify-content: space-between; gap: 6px; margin-top: 3px; }.amountField strong { font-size: 12px; font-variant-numeric: tabular-nums; overflow-wrap: anywhere; }.amountField b { align-self: center; font-size: 9px; color: var(--muted); font-weight: 500; }.amountSlider { display: flex; justify-content: space-between; position: relative; margin: 10px 4px; }.amountSlider::before { content: ''; position: absolute; top: 2px; width: 100%; height: 2px; background: #333c48; }.amountSlider i { z-index: 1; width: 5px; height: 5px; background: #525c6b; transform: rotate(45deg); }.estimateLine { margin-top: 12px; }.feeLine { margin-top: 5px; font-size: 7px; }.tradeAction { display: flex; align-items: center; justify-content: space-between; gap: 5px; margin-top: 12px; padding: 9px 12px; border-radius: 5px; background: var(--green); color: #06281e; font-size: 10px; font-weight: 700; }.tradeAction.sell { background: var(--red); color: #29080e; }.tradeAction svg { width: 15px; height: 15px; }.tradeAction.pressed { transform: translateY(1px); filter: brightness(.9); }
  .focus { position: relative; z-index: 1; outline: 1px solid var(--venue-accent); outline-offset: -1px; box-shadow: 0 0 0 3px color-mix(in srgb, var(--venue-accent) 14%, transparent); }.history { border-top: 1px solid var(--line); background: #0f1217; }.historyTabs { display: flex; gap: 20px; height: 33px; padding: 0 16px; color: var(--muted); font-size: 9px; }.historyTabs > span { display: flex; align-items: center; }.historyTabs .active { color: #f0f3f8; border-bottom: 2px solid var(--venue-accent); }.historyHint { display: flex; justify-content: center; gap: 7px; align-items: center; min-height: 31px; padding: 3px 16px 9px; color: var(--muted); font-size: 8px; }.historyHint > span { font-size: 14px; color: var(--venue-accent); }.cursor { position: absolute; z-index: 3; left: 0; top: 0; width: 23px; height: 28px; filter: drop-shadow(0 2px 3px #0009); pointer-events: none; }.caption { display: flex; align-items: center; gap: 12px; min-height: 66px; padding: 12px 20px; box-sizing: border-box; border-top: 1px solid #303640; background: #171c23; }.captionNumber { display: grid; place-items: center; width: 30px; height: 30px; border: 1px solid color-mix(in srgb, var(--venue-accent) 40%, transparent); border-radius: 8px; color: var(--venue-accent); font: 11px var(--font-mono, monospace); flex-shrink: 0; }.caption > div { display: grid; gap: 4px; }.caption strong { font-size: 12px; }.caption small { color: var(--muted); font-size: 8px; }.captionPair { margin-left: auto; color: var(--muted); font: 10px var(--font-mono, monospace); white-space: nowrap; }
  .watchlist { border-right: 1px solid var(--line); }.watchlist .search { display: flex; gap: 5px; padding: 8px; color: var(--muted); font-size: 9px; }.watchlist > div:not(.panelTitle, .search) { display: flex; align-items: center; gap: 6px; padding: 10px 8px; font-size: 8px; }.watchlist img { width: 14px; height: 14px; }.watchlist small { display: block; color: var(--muted); font-size: 7px; }.watchlist .chosen { background: #468bff18; }.whitebird .workspace { grid-template-columns: 84px minmax(0, 1fr) 120px 218px; }.whitebird .brand strong { font-size: 12px; text-transform: uppercase; letter-spacing: .16em; }.whitebird .tradeAction:not(.sell) { background: var(--venue-accent); color: white; }.binance .terminalHeader, .mexc .terminalHeader { background: #181b21; }.binance .brand strong { color: var(--venue-accent); text-transform: uppercase; }.bitget .brand strong { color: var(--venue-accent); font-size: 18px; }
  @container (min-width: 741px) {
    .binance .workspace { grid-template-columns: 140px minmax(0, 1fr) 140px; }
    .binance .chartPanel { grid-column: 2; grid-row: 1; }
    .binance .orderBook { grid-column: 1; grid-row: 1 / 3; }
    .binance .watchlist { grid-column: 3; grid-row: 1 / 3; border-right: 0; border-left: 1px solid var(--line); }
    .binance .chart { height: 180px; }.binance .candles { height: 151px; }
    .binance .orderPanel { grid-column: 2; grid-row: 2; display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: 8px 16px; padding: 10px 14px 14px; border-top: 1px solid var(--line); }
    .binance .orderPanel > .panelTitle { grid-column: 1; grid-row: 1; height: 26px; border: 0; }
    .binance .sideTabs { grid-column: 2; grid-row: 1; margin-top: 0; }
    .binance .orderTypes { grid-column: 1; grid-row: 2; margin: 0; }
    .binance .balance { grid-column: 2; grid-row: 2; align-items: center; margin: 0; }
    .binance .priceField { grid-column: 1; grid-row: 3; align-items: center; }
    .binance .amountField { grid-column: 2; grid-row: 3; margin: 0; }
    .binance .amountSlider { grid-column: 2; grid-row: 4; margin-block: 4px; }
    .binance .estimateLine { grid-column: 1; grid-row: 4; margin: 0; }
    .binance .feeLine { grid-column: 1; grid-row: 5; align-items: center; margin: 0; }
    .binance .tradeAction { grid-column: 2; grid-row: 5; margin: 0; }
  }
  @container (max-width: 740px) { .workspace { grid-template-columns: minmax(0, 1fr) 110px 210px; }.whitebird .workspace { grid-template-columns: minmax(0, 1fr) 110px 210px; }.watchlist { display: none; }.marketHeader { gap: 20px; }.stat:last-child { display: none; } }
  @container (max-width: 560px) { .terminalHeader { gap: 14px; padding: 0 12px; height: 42px; }.brand strong { font-size: 12px; }.brand img { width: 19px; height: 19px; }.navItem { display: none; }.demoLabel { font-size: 8px; }.marketHeader { gap: 15px; padding: 10px 12px; }.pair strong { font-size: 12px; }.pair img { width: 20px; height: 20px; }.stat { display: none; }.quote small { font-size: 7px; }.workspace, .whitebird .workspace { grid-template-columns: minmax(0, 1fr) minmax(160px, 46%); }.orderBook { display: none; }.orderPanel { padding-inline: 10px; }.chartTabs { padding-inline: 10px; }.chartTabs > span:not(.expand) { display: none; }.intervals { gap: 10px; padding-inline: 10px; }.intervals > span:last-child { display: none; }.chart { padding-inline: 4px; }.chartPair { left: 10px; font-size: 7px; }.chartDisclaimer { left: 10px; font-size: 6px; }.caption { padding-inline: 12px; gap: 9px; }.captionPair { display: none; }.caption strong { font-size: 11px; }.caption small { font-size: 7px; }.historyTabs { gap: 15px; font-size: 8px; }.historyHint { font-size: 7px; } }
  @media (prefers-reduced-motion: reduce) { .cursor { display: none; } }
</style>
