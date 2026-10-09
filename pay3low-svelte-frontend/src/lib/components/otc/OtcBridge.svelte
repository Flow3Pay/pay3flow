<script lang="ts">
  import ExchangeFlowBridge from "../ExchangeFlowBridge.svelte";
  import { locale } from "$lib/i18n";
  import { otcCopy } from "$lib/otc/copy";
  import { formatAmount, formatPrice, prepareDemoOrder, type OtcMarket, type OtcSnapshot, type OrderSide, type OrderType, type OrderDraft } from "$lib/otc/model";
  export let market: OtcMarket;
  export let snapshot: OtcSnapshot;
  export let side: OrderSide = "buy";
  export let type: OrderType = "limit";
  export let price: string;
  export let onReview: (draft: OrderDraft) => void;
  let amount = "1000";
  let previousSide = side;
  $: copy = otcCopy($locale);
  $: if (side !== previousSide) { previousSide = side; amount = side === "buy" ? "1000" : (1000 / market.price).toFixed(market.amountDecimals); }
  $: result = prepareDemoOrder(market, snapshot, side, type, amount, price);
  $: draft = result.draft;
  $: sendAsset = side === "buy" ? market.quote : market.base;
  $: receiveAsset = side === "buy" ? market.base : market.quote;
  $: receive = draft ? side === "buy" ? formatAmount(draft.amount, market) : formatPrice(draft.total, market) : "—";
  $: message = result.error === "liquidity" ? copy.liquidity : result.error === "precision" ? copy.minimum : result.error === "range" ? copy.tooLarge : "";
  $: sendIcon = side === "buy" ? "/icons/assets/usdt.png" : market.icon;
  $: receiveIcon = side === "buy" ? market.icon : "/icons/assets/usdt.png";
  function review() { if (draft) onReview(draft); }
</script>
<form class="bridgeForm" on:submit|preventDefault={review}>
  <div class="sideTabs" role="group" aria-label={copy.side}><button type="button" class:active={side === "buy"} aria-pressed={side === "buy"} on:click={() => side = "buy"}>{copy.buy} {market.base}</button><button type="button" class:active={side === "sell"} class:sellActive={side === "sell"} aria-pressed={side === "sell"} on:click={() => side = "sell"}>{copy.sell} {market.base}</button></div>
  <div class="orderTypes" role="group" aria-label={copy.execution}><button type="button" class:active={type === "limit"} aria-pressed={type === "limit"} on:click={() => type = "limit"}>{copy.limit}</button><button type="button" class:active={type === "market"} aria-pressed={type === "market"} on:click={() => type = "market"}>{copy.marketOrder}</button></div>
  <label class="priceField"><span>{copy.price}</span><div><input aria-label={copy.price} inputmode="decimal" autocomplete="off" bind:value={price} disabled={type === "market"} /><span>{market.quote}</span></div></label>
  <p class="priceHint">{type === "limit" ? copy.priceHint : copy.marketHint}</p>
  <div class="intentLabel"><span>{copy.send}</span></div>
  <div class="moneyPanel"><div class="moneyCopy"><label for="otc-send">{copy.send}</label><input id="otc-send" aria-label={copy.send} inputmode="decimal" autocomplete="off" spellcheck="false" bind:value={amount} on:focus={(event) => event.currentTarget.select()} /></div><div class="asset"><img src={sendIcon} alt="" width="31" height="31" /><strong>{sendAsset}</strong></div></div>
  <ExchangeFlowBridge reversed={side === "sell"} onSwap={() => side = side === "buy" ? "sell" : "buy"} label={`${copy.buy} / ${copy.sell}`} />
  <div class="intentLabel receiveLabel"><span>{copy.receive}</span></div>
  <div class="moneyPanel target"><div class="moneyCopy"><span class="fieldLabel">{copy.receive}</span><output aria-label={copy.receive}>{receive}</output><small>{type === "market" ? copy.estimated : copy.atPrice}</small></div><div class="asset"><img src={receiveIcon} alt="" width="31" height="31" /><strong>{receiveAsset}</strong></div></div>
  <div class="orderDetails"><div><span>{copy.orderValue}</span><strong>{draft ? formatPrice(draft.total, market) : "—"} <small>{market.quote}</small></strong></div><div><span>{copy.execution}</span><strong>{type === "limit" ? copy.atPrice : copy.bestPrice}</strong></div><div><span>{copy.fees}</span><strong>{copy.feesValue}</strong></div></div>
  {#if message}<p class="validation" role="status">{message}</p>{/if}
  <button class="cta" type="submit" disabled={!draft} data-testid="otc-review">{draft ? copy.review : copy.enterAmount}<span aria-hidden="true">↗</span></button>
</form>
<style>
  .bridgeForm { padding: 0 19px 19px; }
  .sideTabs { display: flex; padding: 4px; gap: 4px; border-radius: 9px; background: var(--color-panel); }
  .sideTabs button { width: 50%; padding: 9px 3px; border-radius: 6px; color: var(--color-text-soft); font-size: 12px; font-weight: 750; }
  .sideTabs button.active { background: var(--color-accent); color: #152016; }
  .sideTabs button.sellActive { background: var(--color-accent); color: #152016; }
  .orderTypes { display: flex; gap: 16px; margin: 17px 0 15px; }
  .orderTypes button { position: relative; font-size: 12px; color: var(--color-text-soft); padding-bottom: 8px; }
  .orderTypes button.active { color: var(--color-text); font-weight: 800; }
  .orderTypes button.active::after { content: ""; position: absolute; left: 0; right: 0; bottom: 0; height: 2px; border-radius: 1px; background: var(--color-accent-strong); }
  .priceField { display: block; font-size: 12px; color: var(--color-text-soft); }
  .priceField > div { display: flex; align-items: center; gap: 10px; padding: 10px 12px; margin-top: 6px; background: var(--color-panel); border: 1px solid transparent; border-radius: 7px; }
  .priceField > div:focus-within { border-color: var(--color-accent-strong); }
  .priceField input { width: 100%; min-width: 0; border: 0; outline: 0; background: transparent; font-family: var(--font-mono); font-size: 13px; }
  .priceField input:disabled { opacity: .45; }
  .priceField > div > span { font-family: var(--font-mono); font-size: 12px; }
  .priceHint { min-height: 25px; margin: 6px 0 13px; font-size: 12px; color: var(--color-text-faint); line-height: 1.5; }
  .moneyPanel { display: flex; justify-content: space-between; gap: 8px; align-items: center; min-height: 96px; padding: 15px 15px 12px; border: 1px solid var(--exchange-field-border); background: var(--exchange-field-bg); border-radius: 7px; }
  .moneyCopy { min-width: 0; flex: 1; }
  .moneyCopy label, .fieldLabel { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); white-space: nowrap; }
  .intentLabel { margin: 0 3px 9px; color: var(--color-text-soft); font-family: var(--font-mono); font-size: 12px; font-weight: 500; letter-spacing: .07em; text-transform: uppercase; }
  .intentLabel span::before { display: inline-block; width: 5px; height: 5px; margin-right: 8px; border-radius: 50%; background: var(--color-danger); content: ""; vertical-align: 1px; }
  .receiveLabel span::before { background: var(--color-accent); }
  .moneyCopy input, .moneyCopy output { display: block; width: 100%; min-width: 0; margin: 0 0 5px; font-family: var(--font-mono); font-size: clamp(28px, 3.2vw, 38px); font-weight: 500; letter-spacing: -.05em; line-height: 1.3; background: none; border: 0; outline: 0; text-overflow: ellipsis; }
  .moneyCopy input:focus-visible { outline: 1px solid var(--color-focus); outline-offset: 3px; border-radius: 3px; }
  .moneyCopy output { overflow: hidden; white-space: nowrap; }
  .moneyCopy small { font-size: 12px; color: var(--color-text-faint); }
  .asset { display: flex; align-items: center; gap: 8px; min-width: 95px; padding: 8px; border: 1px solid var(--exchange-asset-border); border-radius: 5px; background: var(--exchange-asset-bg); }
  .asset img { border-radius: 50%; }
  .asset strong { font-size: 14px; font-weight: 800; }
  .orderDetails { display: grid; gap: 10px; padding: 18px 0; }
  .orderDetails > div { display: flex; justify-content: space-between; align-items: center; gap: 10px; font-size: 12px; }
  .orderDetails > div > span { color: var(--color-text-soft); }
  .orderDetails strong { font-weight: 600; font-size: 12px; }
  .orderDetails small { font-size: 12px; font-weight: 400; color: var(--color-text-soft); }
  .cta { display: flex; width: 100%; min-height: 48px; padding: 12px 15px; align-items: center; justify-content: space-between; border: 1px solid #b9e834; background: var(--color-accent); color: #171717; border-radius: 5px; font-size: 12px; font-weight: 800; }
  .cta:hover:not(:disabled) { background: #c9f34b; transform: translateY(-1px); }
  .cta span { font-size: 17px; color: #2e2e2e; }
  :global(html[data-theme="dark"]) .cta { background: var(--color-accent); color: #152016; }
  :global(html[data-theme="dark"]) .cta span { color: #152016; }
  .cta:disabled { opacity: .4; cursor: default; }
  .validation { padding: 0 0 12px; font-size: 12px; color: var(--color-warn); }
  @media (max-width: 640px) { .bridgeForm { padding: 0 16px 16px; } .moneyPanel { flex-wrap: wrap; min-height: 116px; padding-top: 13px; padding-bottom: 10px; } .moneyCopy { flex: none; width: 100%; } .asset { width: 100%; } .sideTabs button { padding: 11px; font-size: 12px; } .orderTypes button { font-size: 12px; min-height: 36px; } .moneyCopy input, .moneyCopy output { font-size: clamp(28px, 3.2vw, 38px); } .moneyCopy label, .fieldLabel, .moneyCopy small, .orderDetails > div, .orderDetails strong { font-size: 12px; } }
</style>
