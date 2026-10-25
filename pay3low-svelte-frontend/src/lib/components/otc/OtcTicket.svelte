<script lang="ts">
  import ExchangeFlowBridge from "../ExchangeFlowBridge.svelte";
  import { locale } from "$lib/i18n";
  import { otcCopy } from "$lib/otc/copy";
  import { rate, decimal, type Direction, type Trade } from "$lib/otc/service";
  import { offerDecimal, type SaleTicketState } from "$lib/otc/public-offers";
  export let sale: SaleTicketState | null = null;
  export let direction: Direction = "buy";
  export let input = "100";
  export let quote: Trade | null = null;
  export let disabled = false, busy = false;
  export let desk = "", status = "";
  export let onDirection: (direction: Direction) => void;
  export let onEdit: () => void;
  export let onRequest: () => void;
  export let onReview: () => void;
  $: copy = otcCopy($locale);
  $: say = (en: string, ru: string) => $locale === "ru" ? ru : en;
  $: sendAsset = sale ? "EVER" : direction === "buy" ? "USDT" : "EVER";
  $: receiveAsset = sale ? "USDT" : direction === "buy" ? "EVER" : "USDT";
  $: sendValue = sale ? sale.everAmount : input;
  $: receiveValue = sale ? sale.usdtAmount : quote?.terms.output || "";
  $: saleStatus = sale ? busy ? say("Please wait for the current wallet or offer request.", "Дождитесь завершения текущего запроса кошелька или оффера.") : sale.linkingEvm ? say("Waiting for your Ethereum wallet address.", "Ожидаем адрес Ethereum-кошелька.") : sale.blockingReason : "";
  function changeSend(value: string) {
    if (sale) sale.onEver(value);
    else { input=value; onEdit(); }
  }
  function changeReceive(value: string) { sale?.onUsdt(value); }

</script>
{#snippet asset(asset: string)}
  <div class="assetIdentity"><img src={asset === "EVER" ? "/icons/assets/ever.svg" : "/icons/assets/usdt.png"} width="22" height="22" alt="" /><strong>{asset}</strong><span>{asset === "EVER" ? "Everscale" : "Ethereum"}</span></div>
{/snippet}
<form class="bridgeForm" on:submit|preventDefault={() => sale ? sale.onRequest() : quote ? onReview() : onRequest()}>
  <div class="sideTabs" class:selling={direction === "sell"} role="group" aria-label="Trade direction"><button type="button" disabled={busy} class:active={direction === "buy"} aria-pressed={direction === "buy"} on:click={() => onDirection("buy")}>{say("Buy EVER", "Купить EVER")}</button><button type="button" disabled={busy} class:active={direction === "sell"} aria-pressed={direction === "sell"} on:click={() => onDirection("sell")}>{say("Sell EVER", "Продать EVER")}</button></div>
  <div class="quoteMode">{sale ? say("Your offer", "Ваше предложение") : say("Fixed desk quote", "Фиксированная котировка деска")}<span>OTC</span></div>
  <label class="priceField"><span>{copy.price} · USDT / EVER</span><div><input aria-label={copy.price} readonly={!sale} value={sale ? sale.price : quote ? rate(quote.terms) : "—"} data-testid={sale ? "otc-offer-price" : undefined} disabled={busy} inputmode="decimal" autocomplete="off" spellcheck="false" placeholder="—" on:input={(event) => sale?.onPrice(event.currentTarget.value)} /><span>USDT</span></div></label>
  <p class="priceHint">{sale ? say("Price is for 1 EVER. Total USDT = EVER amount × price. Changing either amount recalculates the other.", "Price — цена за 1 EVER. Всего USDT = количество EVER × цена. Изменение одной суммы пересчитывает вторую.") : say("The desk quotes your exact input. Whole trades only.", "Деск котирует точную сумму. Только целые сделки.")}</p>
  <div class="intentLabel"><span>{sale ? direction === "buy" ? say("Buy amount", "Количество к покупке") : say("Sell amount", "Количество к продаже") : copy.send}</span></div>
  <div class="moneyPanel exchangeMoneyPanel"><div class="panelCopy"><label for="otc-input">{sale ? direction === "buy" ? say("Buy amount · EVER", "Количество к покупке · EVER") : say("Sell amount · EVER", "Количество к продаже · EVER") : say(`You send · ${sendAsset} on ${sendAsset === "EVER" ? "Everscale" : "Ethereum"}`, `Вы отправляете · ${sendAsset} · ${sendAsset === "EVER" ? "Everscale" : "Ethereum"}`)}</label><input id="otc-input" class="amountInput" type="text" inputmode="decimal" autocomplete="off" spellcheck="false" value={sendValue} data-testid={sale ? "otc-offer-ever" : undefined} disabled={busy} on:input={(event) => changeSend(event.currentTarget.value)} on:focus={(event) => event.currentTarget.select()} /><small>{sale ? say("Amount of EVER, not USDT", "Количество EVER, не сумма USDT") : say("Enter the exact amount", "Укажите точную сумму")}</small></div>{@render asset(sendAsset)}</div>
  <ExchangeFlowBridge reversed={direction === "sell"} onSwap={() => { if (!busy) onDirection(direction === "buy" ? "sell" : "buy"); }} label={`${copy.buy} / ${copy.sell}`} />
  <div class="intentLabel receiveLabel"><span>{sale ? direction === "buy" ? say("Total to pay", "Всего к оплате") : say("Total to receive", "Всего к получению") : copy.receive}</span></div>
  <div class="moneyPanel exchangeMoneyPanel"><div class="panelCopy"><label for="otc-receive">{sale ? direction === "buy" ? say("Total to pay · USDT", "Всего к оплате · USDT") : say("Total to receive · USDT", "Всего к получению · USDT") : say(`You receive · ${receiveAsset} on ${receiveAsset === "EVER" ? "Everscale" : "Ethereum"}`, `Вы получаете · ${receiveAsset} · ${receiveAsset === "EVER" ? "Everscale" : "Ethereum"}`)}</label><input id="otc-receive" class="amountInput amountOutput" readonly={!sale} value={receiveValue} data-testid={sale ? "otc-offer-usdt" : undefined} disabled={busy} inputmode="decimal" autocomplete="off" on:input={(event) => changeReceive(event.currentTarget.value)} placeholder="—" /><small>{sale ? say("Total USDT for the entire offer", "Всего USDT за весь объём EVER") : quote ? say("Fixed net output", "Фиксированная сумма к получению") : say("No quote yet", "Котировки пока нет")}</small></div>{@render asset(receiveAsset)}</div>
  {#if sale}
    {#if sale.askingRate !== "—"}<p class="priceHint offerEquation" data-testid="otc-offer-equation">{offerDecimal(sale.everAmount,9)} EVER × {sale.askingRate} USDT / EVER {sale.priceRounded ? "≈" : "="} {offerDecimal(sale.usdtAmount,6)} USDT</p>{/if}
    <label class="recipientField">{direction === "buy" ? say("Your Ethereum address for paying USDT", "Ваш Ethereum-адрес для оплаты USDT") : say("Your Ethereum address for receiving USDT", "Ваш Ethereum-адрес для получения USDT")}<input data-testid="otc-offer-recipient" value={sale.receivingAddress} on:input={(event) => sale?.onAddress(event.currentTarget.value)} autocomplete="off" spellcheck="false" placeholder="0x…" disabled={busy} /></label>
    <button class="walletAddress" type="button" disabled={busy || sale.linkingEvm} on:click={sale.onUseEthereum}>{sale.linkingEvm ? say("Connecting…", "Подключаем…") : say("Use my Ethereum wallet address", "Использовать адрес моего Ethereum-кошелька")}</button>
    {#if sale.priceRounded}<p class="priceHint">{sale.roundingAsset === "EVER" ? say("The EVER amount is rounded down to 9 decimals. Review the exact amounts before publishing.", "Сумма EVER округлена вниз до 9 знаков. Проверьте точные суммы перед публикацией.") : say("The total is rounded down to 6 USDT decimal places. Review the exact amount before publishing.", "Общая сумма округлена вниз до 6 знаков USDT. Проверьте точную сумму перед публикацией.")}</p>{/if}
    {#if sale.reviewing && sale.seller}<div class="saleReview" data-testid="otc-offer-review"><p class="reviewPending">{say("Not published yet. Check the terms and click Publish offer below.", "Ещё не опубликовано. Проверьте условия и нажмите «Опубликовать предложение» ниже.")}</p><strong>{direction === "buy" ? say("Buy", "Купить") : say("Sell", "Продать")} {offerDecimal(sale.everAmount,9)} EVER · {offerDecimal(sale.usdtAmount,6)} USDT</strong><p>{say("Price", "Курс")}: {sale.askingRate} USDT / EVER</p><p>{direction === "buy" ? say("Your EVER receiving wallet", "Ваш кошелёк для получения EVER") : say("Seller", "Продавец")}: <code>{sale.seller}</code></p><p>{direction === "buy" ? say("USDT paying wallet", "Кошелёк оплаты USDT") : say("USDT recipient", "Получатель USDT")}: <code>{sale.receivingAddress.trim()}</code></p><p>{say("These amounts and addresses will be public for 24 hours, or until you cancel. Publication does not move or reserve funds.", "Эти суммы и адреса будут опубликованы на 24 часа или до отмены. Публикация не переводит и не резервирует средства.")}</p></div>{/if}
  {:else}
  <div class="orderDetails"><div><span>{say("Desk", "Деск")}</span><strong>{desk || "—"}</strong></div><div><span>{copy.fees}</span><strong>{quote ? `${decimal(quote.terms.fee_policy.fee_units, 6)} USDT` : "max(0.25%, 5 USDT)"}</strong></div><small>{say("Completion fee paid by the desk. Network costs are separate.", "Комиссию за завершение платит деск. Сетевые расходы отдельно.")}</small></div>
  {/if}
  {#if quote}<p class="priceHint">{say("Quote expires", "Котировка истекает")}: {quote.terms.quote_by}</p>{/if}
  {#if status}<p class="priceHint" id="otc-ticket-status" role="status">{status}</p>{/if}
  {#if saleStatus}<p class="priceHint" id="otc-sale-status" role="status">{saleStatus}</p>{/if}
  <button aria-describedby={saleStatus ? "otc-sale-status" : status ? "otc-ticket-status" : undefined} class="cta" type="submit" disabled={busy || (sale ? sale.linkingEvm || (!!sale.seller && !sale.valid) : disabled)} data-testid={sale ? sale.seller ? "otc-publish-offer" : "otc-connect-offer" : "otc-review"}>{sale ? !sale.seller ? say("Connect EVER Wallet to publish", "Подключить EVER Wallet для публикации") : sale.reviewing ? say("Publish offer", "Опубликовать предложение") : say("Review offer", "Проверить предложение") : quote ? say("Review fixed quote", "Проверить котировку") : say("Request exact quote", "Запросить котировку")}<span aria-hidden="true">↗</span></button>
</form>

<style>
  .recipientField { display:grid; gap:8px; margin:16px 0 8px; color:var(--color-text-soft); font-size:12px; }
  .recipientField input { width:100%; min-width:0; min-height:44px; padding:10px 12px; border:1px solid var(--exchange-field-border); border-radius:8px; background:var(--exchange-field-bg); color:var(--color-text); font:inherit; }
  .walletAddress { min-height:44px; padding:8px 12px; border:1px solid var(--color-border); border-radius:8px; font-size:12px; }
  .saleReview { margin-bottom:16px; padding:12px; border:1px solid var(--color-border); border-radius:8px; }
  .saleReview .reviewPending { margin:0 0 12px; color:var(--color-text); font-weight:600; }
  .saleReview p { margin-top:8px; color:var(--color-text-soft); font-size:12px; line-height:1.6; }
  .saleReview code { overflow-wrap:anywhere; font-size:11px; }
  .bridgeForm { display: flex; flex: 1; flex-direction: column; min-width: 0; padding: 0 var(--otc-panel-padding, 20px) var(--otc-panel-padding, 20px); }
  .sideTabs { position: relative; isolation: isolate; display: flex; flex: 0 0 auto; gap: 4px; padding: 4px; border-radius: 8px; background: var(--color-panel); }
  .sideTabs::before { position: absolute; z-index: -1; inset: 4px auto 4px 4px; width: calc(50% - 6px); border-radius: 6px; background: var(--color-accent); content: ""; transition: transform .28s cubic-bezier(.22, 1, .36, 1); }
  .sideTabs.selling::before { transform: translateX(calc(100% + 4px)); }
  .sideTabs button { flex: 1; min-width: 0; min-height: 40px; padding: 8px 4px; border-radius: 6px; color: var(--color-text-soft); font-size: 13px; font-weight: 650; line-height: 1.5; transition: color .2s ease; }
  .sideTabs button.active { color: #152016; }
  .quoteMode { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 16px 0; font-size: 12px; line-height: 1.5; }
  .quoteMode span { flex: 0 0 auto; color: var(--color-text-soft); font-family: var(--font-mono); }
  .priceField { display: block; color: var(--color-text-soft); font-size: 12px; }
  .priceField > div { display: flex; min-height: 44px; align-items: center; gap: 8px; margin-top: 8px; padding: 8px 12px; border: 1px solid var(--exchange-field-border); border-radius: 8px; background: var(--exchange-field-bg); }
  .priceField input { width: 100%; min-width: 0; min-height: 24px; height: 24px; padding: 0; border: 0; outline: 0; background: transparent; font-family: var(--font-mono); font-size: 16px; }
  .priceField > div > span { flex: 0 0 auto; font-family: var(--font-mono); font-size: 12px; }
  .priceHint { margin: 8px 0 16px; color: var(--color-text-soft); font-size: 12px; line-height: 1.6; overflow-wrap: anywhere; }
  .intentLabel { margin: 0 0 8px; color: var(--color-text-soft); font-family: var(--font-mono); font-size: 12px; font-weight: 400; letter-spacing: .04em; text-transform: uppercase; }
  .intentLabel span::before { display: inline-block; width: 5px; height: 5px; margin-right: 8px; border-radius: 50%; background: var(--color-danger); content: ""; vertical-align: 2px; }
  .receiveLabel span::before { background: var(--color-accent-strong); }
  .moneyPanel { display: grid; flex: 0 0 auto; grid-template-columns: minmax(0, 1fr); min-height: 0; gap: 8px; padding: 12px; border: 1px solid var(--exchange-field-border); border-radius: 8px; }
  .moneyPanel:hover, .moneyPanel:focus-within { border-width: 1px; }
  .moneyPanel:hover:not(:focus-within) { outline: none; }
  .moneyPanel .panelCopy { display: grid; flex: none; width: 100%; grid-template-rows: 38px 18px; gap: 4px; }
  .moneyPanel .amountInput { min-height: 38px; height: 38px; padding: 0; font-size: 32px; line-height: 1.15; letter-spacing: -.04em; }
  .panelCopy small { overflow: hidden; color: var(--color-text-soft); font-size: 12px; line-height: 18px; text-overflow: ellipsis; white-space: nowrap; }
  .assetIdentity { display: flex; min-width: 0; min-height: 40px; align-items: center; gap: 8px; padding: 8px 10px; border: 1px solid var(--exchange-asset-border); border-radius: 6px; background: var(--exchange-asset-bg); font-size: 13px; line-height: 1.5; }
  .assetIdentity img { flex: 0 0 22px; }
  .assetIdentity strong { font-weight: 650; }
  .assetIdentity span { min-width: 0; margin-left: auto; overflow-wrap: anywhere; color: var(--color-text-soft); font-size: 12px; }
  .orderDetails { display: grid; gap: 8px; padding: 16px 0; }
  .orderDetails > div { display: flex; min-width: 0; align-items: baseline; justify-content: space-between; gap: 12px; font-size: 12px; }
  .orderDetails > div > span { flex: 0 0 auto; color: var(--color-text-soft); }
  .orderDetails strong { min-width: 0; overflow-wrap: anywhere; font-size: 12px; font-weight: 600; text-align: right; }
  .orderDetails small { color: var(--color-text-soft); font-size: 12px; font-weight: 400; line-height: 1.6; }
  .cta { display: flex; width: 100%; min-height: 48px; align-items: center; justify-content: space-between; gap: 12px; margin-top: auto; padding: 12px 16px; border: 1px solid var(--color-accent-strong); border-radius: 8px; background: var(--color-accent); color: #152016; font-size: 13px; font-weight: 750; line-height: 1.5; text-align: left; transition: background .18s ease, opacity .18s ease; }
  .cta:hover:not(:disabled) { background: #c9f34b; }
  .cta span { flex: 0 0 auto; font-size: 18px; }
  .cta:disabled { opacity: .45; cursor: default; }
  button:focus-visible, .priceField > div:focus-within { outline: var(--focus-ring-width) solid var(--color-focus); outline-offset: 2px; }
  @media (max-width: 640px) { .moneyPanel .amountInput { font-size: 28px; } .sideTabs button { min-height: 44px; } }
  @media (prefers-reduced-motion: reduce) { .sideTabs::before, .sideTabs button, .cta { transition: none; } }
</style>
