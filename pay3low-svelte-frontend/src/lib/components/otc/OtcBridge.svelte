<script lang="ts">
  import ExchangeFlowBridge from "../ExchangeFlowBridge.svelte";
  import PaymentMethodPicker from "../PaymentMethodPicker.svelte";
  import NetworkPicker from "../NetworkPicker.svelte";
  import { networkIcon } from "$lib/icons";
  import { locale } from "$lib/i18n";
  import { otcCopy } from "$lib/otc/copy";
  import type { CryptoNetwork } from "$lib/networks";
  import type { PaymentMethod } from "$lib/payment-methods";
  import { markets, formatAmount, formatPrice, prepareDemoOrder, type OtcMarket, type OtcSnapshot, type OrderSide, type OrderType, type OrderDraft } from "$lib/otc/model";
  export let disabled = false;
  export let market: OtcMarket;
  export let snapshot: OtcSnapshot;
  export let networks: CryptoNetwork[];
  export let sendNetworkId: string;
  export let receiveNetworkId: string;
  export let onSelectAsset: (asset: string, field: "send" | "receive", network: CryptoNetwork) => void;
  export let onSideChange: (side: OrderSide) => void;
  export let side: OrderSide = "buy";
  export let type: OrderType = "limit";
  export let price: string;
  export let onReview: (draft: OrderDraft) => void;
  export let amount: string;
  let assetPicker: "send" | "receive" | null = null;
  let networkPicker: "send" | "receive" | null = null;
  const assetMethods: PaymentMethod[] = [
    { id: "otc-usdt", name: "Tether", currency: "USDT", country: "global", role: "both", kind: "wallet", initials: "USDT", color: "#26a17b", iconUrl: "/icons/assets/usdt.png", p2pQuery: "USDT" },
    ...markets.map((item): PaymentMethod => ({ id: `otc-${item.base.toLowerCase()}`, name: item.name, currency: item.base, country: "global", role: "both", kind: "wallet", initials: item.base, color: "#171a17", iconUrl: item.icon, p2pQuery: item.base })),
  ];
  $: copy = otcCopy($locale);
  $: result = prepareDemoOrder(market, snapshot, side, type, amount, price);
  $: draft = result.draft;
  $: sendAsset = side === "buy" ? market.quote : market.base;
  $: receiveAsset = side === "buy" ? market.base : market.quote;
  $: sendNetworks = networks.filter((item) => item.currencies.includes(sendAsset));
  $: receiveNetworks = networks.filter((item) => item.currencies.includes(receiveAsset));
  const nativeNetwork: Record<string, string> = { BTC: "bitcoin", ETH: "ethereum", SOL: "solana", USDT: "ethereum" };
  $: sendNetwork = sendNetworks.find((item) => item.id === sendNetworkId) ?? sendNetworks.find((item) => item.id === nativeNetwork[sendAsset]) ?? sendNetworks[0];
  $: receiveNetwork = receiveNetworks.find((item) => item.id === receiveNetworkId) ?? receiveNetworks.find((item) => item.id === nativeNetwork[receiveAsset]) ?? receiveNetworks[0];
  $: syncNetworkIds(sendNetwork, receiveNetwork);
  function syncNetworkIds(send: CryptoNetwork | undefined, receive: CryptoNetwork | undefined) {
    if (send && send.id !== sendNetworkId) sendNetworkId = send.id;
    if (receive && receive.id !== receiveNetworkId) receiveNetworkId = receive.id;
  }
  $: sendMethod = assetMethods.find((item) => item.currency === sendAsset)!;
  $: receiveMethod = assetMethods.find((item) => item.currency === receiveAsset)!;
  $: receive = draft ? side === "buy" ? formatAmount(draft.amount, market) : formatPrice(draft.total, market) : "";
  $: message = result.error === "liquidity" ? copy.liquidity : result.error === "precision" ? copy.minimum : result.error === "range" ? copy.tooLarge : "";
  function review() {
    if (!disabled && draft && sendNetwork && receiveNetwork) onReview({ ...draft, sendNetwork: sendNetwork.name, receiveNetwork: receiveNetwork.name });
  }
  function selectAsset(method: PaymentMethod, network?: CryptoNetwork) {
    if (!assetPicker || !network) return;
    const field = assetPicker;
    assetPicker = null;
    onSelectAsset(method.currency, field, network);
  }
  function selectNetwork(network: CryptoNetwork) {
    if (networkPicker === "send") sendNetworkId = network.id;
    else receiveNetworkId = network.id;
    networkPicker = null;
  }
</script>

{#snippet assetControls(field: "send" | "receive", method: PaymentMethod, network: CryptoNetwork | undefined)}
  <div class="methodControls">
    <button type="button" class="methodTrigger" aria-haspopup="dialog" aria-label={`${field === "send" ? copy.selectSendAsset : copy.selectReceiveAsset}: ${method.currency}`} title={`${method.currency} · ${method.name}`} on:click={() => assetPicker = field}>
      <span class="methodAvatar" aria-hidden="true"><img src={method.iconUrl} alt="" width="28" height="28" /></span>
      <span class="methodText methodTextAsset"><strong>{method.currency}</strong></span>
      <svg width="15" height="15" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="m4 6 4 4 4-4" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" /></svg>
    </button>
    <div class="networkControl"><button type="button" class="networkButton" aria-haspopup="dialog" aria-label={`${field === "send" ? copy.selectSendNetwork : copy.selectReceiveNetwork}: ${network?.name ?? "—"}`} title={network?.name} on:click={() => networkPicker = field}>
      <span class="networkDot" aria-hidden="true"><img src={networkIcon(network?.name)} alt="" width="18" height="18" /></span>
      <svg class="networkChevron" width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="m4 6 4 4 4-4" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" /></svg>
    </button></div>
  </div>
{/snippet}
<form class="bridgeForm" on:submit|preventDefault={review}>
  <div class="sideTabs" class:selling={side === "sell"} role="group" aria-label={copy.side}><button type="button" class:active={side === "buy"} aria-pressed={side === "buy"} on:click={() => onSideChange("buy")}>{copy.buy}</button><button type="button" class:active={side === "sell"} aria-pressed={side === "sell"} on:click={() => onSideChange("sell")}>{copy.sell}</button></div>
  <div class="orderTypes" class:marketOrder={type === "market"} role="group" aria-label={copy.execution}><button type="button" class:active={type === "limit"} aria-pressed={type === "limit"} on:click={() => type = "limit"}>{copy.limit}</button><button type="button" class:active={type === "market"} aria-pressed={type === "market"} on:click={() => type = "market"}>{copy.marketOrder}</button></div>
  <label class="priceField"><span>{copy.price}</span><div><input aria-label={copy.price} inputmode="decimal" autocomplete="off" bind:value={price} disabled={type === "market"} /><span>{market.quote}</span></div></label>
  <p class="priceHint">{type === "limit" ? copy.priceHint : copy.marketHint}</p>
  <div class="intentLabel"><span>{copy.send}</span></div>
  <div class="moneyPanel exchangeMoneyPanel"><div class="panelCopy"><label for="otc-send">{copy.send}</label><input id="otc-send" class="amountInput" type="text" aria-label={copy.send} inputmode="decimal" autocomplete="off" spellcheck="false" bind:value={amount} on:focus={(event) => event.currentTarget.select()} /></div>{@render assetControls("send", sendMethod, sendNetwork)}</div>
  <ExchangeFlowBridge reversed={side === "sell"} onSwap={() => onSideChange(side === "buy" ? "sell" : "buy")} label={`${copy.buy} / ${copy.sell}`} />
  <div class="intentLabel receiveLabel"><span>{copy.receive}</span></div>
  <div class="moneyPanel exchangeMoneyPanel"><div class="panelCopy"><label for="otc-receive">{copy.receive}</label><input id="otc-receive" class="amountInput amountOutput" type="text" readonly value={receive} aria-label={copy.receive} /><small>{type === "market" ? copy.estimated : copy.atPrice}</small></div>{@render assetControls("receive", receiveMethod, receiveNetwork)}</div>
  <div class="orderDetails"><div><span>{copy.orderValue}</span><strong>{draft ? formatPrice(draft.total, market) : "—"} <small>{market.quote}</small></strong></div><div><span>{copy.execution}</span><strong>{type === "limit" ? copy.atPrice : copy.bestPrice}</strong></div><div><span>{copy.fees}</span><strong>{copy.feesValue}</strong></div></div>
  {#if message}<p class="validation" role="status">{message}</p>{/if}
  <button class="cta" type="submit" disabled={disabled || !draft || !sendNetwork || !receiveNetwork} data-testid="otc-review">{draft ? copy.review : copy.enterAmount}<span aria-hidden="true">↗</span></button>
</form>
{#if assetPicker}
  <PaymentMethodPicker open title={assetPicker === "send" ? copy.chooseSendAsset : copy.chooseReceiveAsset} role={assetPicker === "send" ? "sender" : "recipient"} {networks} paymentMethods={assetMethods} selected={assetPicker === "send" ? sendMethod : receiveMethod} selectedNetwork={assetPicker === "send" ? sendNetwork : receiveNetwork} onClose={() => assetPicker = null} onSelect={selectAsset} />
{/if}
{#if networkPicker}
  <NetworkPicker open networks={networkPicker === "send" ? sendNetworks : receiveNetworks} selected={networkPicker === "send" ? sendNetwork : receiveNetwork} onClose={() => networkPicker = null} onSelect={selectNetwork} />
{/if}
<style>
  .bridgeForm { container-type: inline-size; container-name: exchange-bridge; padding: 0 19px 19px; }
  .orderTypes { display: flex; gap: 16px; margin: 17px 0 15px; }
  .orderTypes button { position: relative; font-size: 12px; color: var(--color-text-soft); padding-bottom: 8px; }
  .orderTypes button.active { color: var(--color-text); font-weight: 800; }
  .orderTypes button.active::after { content: ""; position: absolute; left: 0; right: 0; bottom: 0; height: 2px; border-radius: 1px; background: var(--color-accent-strong); }
  .priceField { display: block; font-size: 12px; color: var(--color-text-soft); }
  .priceField > div { display: flex; align-items: center; gap: 10px; padding: 10px 12px; margin-top: 6px; background: var(--exchange-field-bg); border: 1px solid var(--exchange-field-border); border-radius: 7px; }
  .priceField > div:focus-within { border-color: var(--color-accent-strong); }
  .priceField input { width: 100%; min-width: 0; border: 0; outline: 0; background: transparent; font-family: var(--font-mono); font-size: 16px; }
  .priceField input:disabled { opacity: .45; }
  .priceField > div > span { font-family: var(--font-mono); font-size: 12px; }
  .priceHint { min-height: 25px; margin: 6px 0 13px; font-size: 12px; color: var(--color-text-faint); line-height: 1.5; }
  .intentLabel { margin: 0 3px 9px; color: var(--color-text-soft); font-family: var(--font-mono); font-size: 12px; font-weight: 500; letter-spacing: .07em; text-transform: uppercase; }
  .intentLabel span::before { display: inline-block; width: 5px; height: 5px; margin-right: 8px; border-radius: 50%; background: var(--color-danger); content: ""; vertical-align: 1px; }
  .receiveLabel span::before { background: var(--color-accent); }
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
  .sideTabs { position: relative; isolation: isolate; display: flex; padding: 4px; gap: 4px; border-radius: 9px; background: var(--color-panel); }
  .sideTabs::before { position: absolute; z-index: -1; inset: 4px auto 4px 4px; width: calc(50% - 6px); border-radius: 6px; background: var(--color-accent); content: ""; transition: transform .32s cubic-bezier(.22, 1, .36, 1); }
  .sideTabs.selling::before { transform: translateX(calc(100% + 4px)); }
  .sideTabs button.active { color: #152016; }
  .sideTabs button { flex: 1; min-width: 0; padding: 9px 3px; border-radius: 6px; color: var(--color-text-soft); font-size: 12px; font-weight: 750; transition: color .2s ease; }
  .priceField > div { min-height: 44px; transition: border-color .18s ease, background .18s ease; }
  .priceField > div:focus-within { outline: var(--focus-ring-width) solid var(--color-focus); outline-offset: 2px; }
  .panelCopy small { font-size: 12px; color: var(--color-text-faint); }
  .cta { transition: background .18s ease, transform .18s ease, opacity .18s ease; }
  @media (max-width: 640px) { .bridgeForm { padding: 0 16px 16px; } .sideTabs button { padding: 11px; } .orderTypes button { min-height: 36px; } }
  @media (prefers-reduced-motion: reduce) { .sideTabs::before, .sideTabs button, .priceField > div, .cta { transition: none; } }

</style>
