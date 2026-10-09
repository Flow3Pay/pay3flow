<script lang="ts">
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
  <div class="moneyPanel"><div class="moneyCopy"><label for="otc-send">{copy.send}</label><input id="otc-send" aria-label={copy.send} inputmode="decimal" autocomplete="off" spellcheck="false" bind:value={amount} on:focus={(event) => event.currentTarget.select()} /><small>{copy.direct}</small></div><div class="asset"><img src={sendIcon} alt="" width="31" height="31" /><strong>{sendAsset}</strong></div></div>
  <div class="flowBridge"><span></span><button type="button" aria-label={`${copy.buy} / ${copy.sell}`} on:click={() => side = side === "buy" ? "sell" : "buy"}><svg width="18" height="18" viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="M8 4v16m-4-4 4 4 4-4m4-12v16m-4-12 4-4 4 4" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" /></svg></button><span></span></div>
  <div class="moneyPanel target"><div class="moneyCopy"><span class="fieldLabel">{copy.receive}</span><output aria-label={copy.receive}>{receive}</output><small>{type === "market" ? copy.estimated : copy.atPrice}</small></div><div class="asset"><img src={receiveIcon} alt="" width="31" height="31" /><strong>{receiveAsset}</strong></div></div>
  <div class="orderDetails"><div><span>{copy.orderValue}</span><strong>{draft ? formatPrice(draft.total, market) : "—"} <small>{market.quote}</small></strong></div><div><span>{copy.execution}</span><strong>{type === "limit" ? copy.atPrice : copy.bestPrice}</strong></div><div><span>{copy.fees}</span><strong>{copy.feesValue}</strong></div></div>
  {#if message}<p class="validation" role="status">{message}</p>{/if}
  <button class="cta" type="submit" disabled={!draft} data-testid="otc-review">{draft ? copy.review : copy.enterAmount}<span aria-hidden="true">↗</span></button>
  <p class="orderNote"><svg width="13" height="13" viewBox="0 0 24 24" fill="none" aria-hidden="true"><rect x="6" y="10" width="12" height="10" rx="2" stroke="currentColor" stroke-width="1.5" /><path d="M8 10V7a4 4 0 0 1 8 0v3" stroke="currentColor" stroke-width="1.5" /></svg>{copy.demoOnly}</p>
</form>
<style>
  .bridgeForm { padding: 16px; }
  .sideTabs { display: flex; padding: 4px; gap: 4px; border-radius: 9px; background: var(--color-panel); }
  .sideTabs button { width: 50%; padding: 9px 3px; border-radius: 6px; color: var(--color-text-soft); font-size: 11px; font-weight: 750; }
  .sideTabs button.active { background: var(--color-paper); color: var(--otc-buy); box-shadow: 0 1px 5px #0000000a; }
  .sideTabs button.sellActive { color: var(--otc-sell); }
  .orderTypes { display: flex; gap: 16px; margin: 17px 0 15px; }
  .orderTypes button { position: relative; font-size: 10px; color: var(--color-text-soft); padding-bottom: 8px; }
  .orderTypes button.active { color: var(--color-text); font-weight: 800; }
  .orderTypes button.active::after { content: ""; position: absolute; left: 0; right: 0; bottom: 0; height: 2px; border-radius: 1px; background: var(--color-accent-strong); }
  .priceField { display: block; font-size: 10px; color: var(--color-text-soft); }
  .priceField > div { display: flex; align-items: center; gap: 10px; padding: 10px 12px; margin-top: 6px; background: var(--color-panel); border: 1px solid transparent; border-radius: 8px; }
  .priceField > div:focus-within { border-color: var(--color-accent-strong); }
  .priceField input { width: 100%; min-width: 0; border: 0; outline: 0; background: transparent; font-family: var(--font-mono); font-size: 13px; }
  .priceField input:disabled { opacity: .45; }
  .priceField > div > span { font-family: var(--font-mono); font-size: 9px; }
  .priceHint { min-height: 25px; margin: 6px 0 13px; font-size: 9px; color: var(--color-text-faint); line-height: 1.5; }
  .moneyPanel { display: flex; justify-content: space-between; gap: 8px; align-items: center; padding: 14px 12px; background: var(--color-panel); border-radius: 11px; }
  .moneyCopy { min-width: 0; flex: 1; }
  .moneyCopy label, .fieldLabel { display: block; font-size: 9px; color: var(--color-text-soft); }
  .moneyCopy input, .moneyCopy output { display: block; width: 100%; min-width: 0; margin: 7px 0 5px; font-family: var(--font-mono); font-size: 23px; letter-spacing: -.06em; line-height: 1.3; background: none; border: 0; outline: 0; text-overflow: ellipsis; }
  .moneyCopy input:focus-visible { outline: 1px solid var(--color-focus); outline-offset: 3px; border-radius: 3px; }
  .moneyCopy output { overflow: hidden; white-space: nowrap; }
  .moneyCopy small { font-size: 9px; color: var(--color-text-faint); }
  .asset { display: grid; justify-items: center; gap: 5px; min-width: 46px; }
  .asset img { border-radius: 50%; }
  .asset strong { font-size: 11px; }
  .flowBridge { display: flex; align-items: center; gap: 8px; padding: 5px 6px; }
  .flowBridge > span { flex: 1; height: 1px; background: var(--color-border); }
  .flowBridge button { display: grid; width: 28px; height: 28px; border: 1px solid var(--color-border); border-radius: 8px; place-items: center; background: var(--color-panel); }
  .flowBridge button:hover { background: var(--color-accent-soft); }
  .orderDetails { display: grid; gap: 10px; padding: 18px 0; }
  .orderDetails > div { display: flex; justify-content: space-between; align-items: center; gap: 10px; font-size: 9px; }
  .orderDetails > div > span { color: var(--color-text-soft); }
  .orderDetails strong { font-weight: 600; font-size: 9px; }
  .orderDetails small { font-size: 8px; font-weight: 400; color: var(--color-text-soft); }
  .cta { display: flex; width: 100%; min-height: 45px; padding: 12px 15px; align-items: center; justify-content: space-between; background: var(--color-primary); color: var(--color-text-invert); border-radius: 9px; font-size: 12px; font-weight: 800; }
  .cta:hover { background: var(--color-primary-strong); }
  .cta span { font-size: 18px; color: var(--color-accent); }
  :global(html[data-theme="dark"]) .cta { background: var(--color-accent); color: #152016; }
  :global(html[data-theme="dark"]) .cta span { color: #152016; }
  .cta:disabled { opacity: .4; cursor: default; }
  .orderNote { display: flex; justify-content: center; align-items: flex-start; gap: 5px; margin-top: 12px; font-size: 8px; line-height: 1.5; color: var(--color-text-faint); }
  .orderNote svg { flex-shrink: 0; }
  .validation { padding: 0 0 12px; font-size: 10px; color: var(--color-warn); }
  @media (max-width: 640px) { .sideTabs button { padding: 11px; font-size: 12px; } .orderTypes button { font-size: 12px; min-height: 36px; } .moneyCopy input, .moneyCopy output { font-size: 27px; } .moneyCopy label, .fieldLabel, .moneyCopy small, .orderDetails > div, .orderDetails strong { font-size: 11px; } .orderNote { font-size: 9px; } }
</style>
