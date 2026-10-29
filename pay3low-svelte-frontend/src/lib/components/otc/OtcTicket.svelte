<script lang="ts">
  import { tick } from "svelte";
  import ExchangeFlowBridge from "../ExchangeFlowBridge.svelte";
  import { locale } from "$lib/i18n";
  import { otcCopy } from "$lib/otc/copy";
  import { rate, type Direction, type Trade, type WalletBalance } from "$lib/otc/service";
  import { offerDecimal, type SaleTicketState } from "$lib/otc/public-offers";
  import { trimDecimal } from "$lib/otc/market";
  export let sale: SaleTicketState | null = null;
  export let direction: Direction = "buy";
  export let input = "100";
  export let quote: Trade | null = null;
  export let disabled = false, busy = false;
  export let status = "";
  export let balances: WalletBalance[] = [];
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
  $: balanceOf = (asset: string) => {
    const entry = balances.find(balance => balance.symbol === asset);
    return entry?.state === "ok" && entry.amount ? trimDecimal(entry.amount, asset === "EVER" ? 4 : 2) : "—";
  };
  let settingsOpen = false, settingsWrap: HTMLDivElement, settingsInput: HTMLInputElement;
  function openSettings() {
    settingsOpen = true;
    void tick().then(() => settingsInput?.focus());
  }
  function closeSettings(restoreFocus = false) {
    settingsOpen = false;
    if (restoreFocus) settingsWrap?.querySelector<HTMLButtonElement>(".settingsButton")?.focus();
  }
  function outsideSettings(event: PointerEvent) {
    if (settingsOpen && settingsWrap && !settingsWrap.contains(event.target as Node)) closeSettings();
  }
  function settingsKeys(event: KeyboardEvent) {
    if (settingsOpen && event.key === "Escape") { event.preventDefault(); closeSettings(true); }
  }
  function changeSend(value: string) {
    if (sale) sale.onEver(value);
    else { input=value; onEdit(); }
  }
  function changeReceive(value: string) { sale?.onUsdt(value); }

</script>
{#snippet asset(asset: string)}
  <div class="assetIdentity"><img src={asset === "EVER" ? "/icons/assets/ever.svg" : "/icons/assets/usdt.png"} width="22" height="22" alt="" /><strong>{asset}</strong><span>{asset === "EVER" ? "Everscale" : "Ethereum"}</span></div>
{/snippet}
{#snippet stack(role: string, asset: string)}
  <div class="balanceStack"><span>{role}</span><b>{say("Balance", "Баланс")}</b><i>{balanceOf(asset)}</i></div>
{/snippet}
<svelte:window on:pointerdown={outsideSettings} on:keydown={settingsKeys} />
<form class="bridgeForm" on:submit|preventDefault={() => sale ? sale.onRequest() : quote ? onReview() : onRequest()}>
  <div class="ticketHead">
    <div class="sideTabs" class:selling={direction === "sell"} role="group" aria-label="Trade direction"><button type="button" disabled={busy} class:active={direction === "buy"} aria-pressed={direction === "buy"} on:click={() => { closeSettings(); onDirection("buy"); }}>{say("Buy EVER", "Купить EVER")}</button><button type="button" disabled={busy} class:active={direction === "sell"} aria-pressed={direction === "sell"} on:click={() => { closeSettings(); onDirection("sell"); }}>{say("Sell EVER", "Продать EVER")}</button></div>
    {#if sale}
      <div class="settingsWrap" bind:this={settingsWrap}>
        <button type="button" class="settingsButton" class:active={settingsOpen} aria-haspopup="dialog" aria-expanded={settingsOpen} aria-label={say("Offer settings", "Настройки предложения")} title={say("Offer settings", "Настройки предложения")} on:click={() => settingsOpen ? closeSettings() : openSettings()}><img src="/icons/ui/route-settings.png" alt="" width="18" height="18" /></button>
        {#if settingsOpen}
          <div class="settingsPanel" role="dialog" aria-label={say("Offer settings", "Настройки предложения")} tabindex="-1">
            <div class="settingsPanelHead"><strong>{say("Offer settings", "Настройки предложения")}</strong><button type="button" class="settingsClose" aria-label={say("Close settings", "Закрыть настройки")} on:click={() => closeSettings(true)}>×</button></div>
            <label class="recipientField">{direction === "buy" ? say("Your Ethereum address for paying USDT", "Ваш Ethereum-адрес для оплаты USDT") : say("Your Ethereum address for receiving USDT", "Ваш Ethereum-адрес для получения USDT")}<input bind:this={settingsInput} data-testid="otc-offer-recipient" value={sale.receivingAddress} on:input={(event) => sale?.onAddress(event.currentTarget.value)} autocomplete="off" spellcheck="false" placeholder="0x…" disabled={busy} /></label>
            <button class="walletAddress" type="button" disabled={busy || sale.linkingEvm} on:click={sale.onUseEthereum}>{sale.linkingEvm ? say("Connecting…", "Подключаем…") : say("Use my Ethereum wallet address", "Использовать адрес моего Ethereum-кошелька")}</button>
          </div>
        {/if}
      </div>
    {/if}
  </div>
  <div class="quoteMode">{sale ? say("Your offer", "Ваше предложение") : say("Fixed desk quote", "Фиксированная котировка деска")}</div>
  <div class="moneyPanel exchangeMoneyPanel"><div class="panelTop">{@render asset(sendAsset)}{@render stack(direction === "buy" ? say("You pay", "Платите") : say("You sell", "Продаёте"), sendAsset)}</div><div class="panelCopy"><label for="otc-input">{sale ? direction === "buy" ? say("Buy amount · EVER", "Количество к покупке · EVER") : say("Sell amount · EVER", "Количество к продаже · EVER") : say(`You send · ${sendAsset} on ${sendAsset === "EVER" ? "Everscale" : "Ethereum"}`, `Вы отправляете · ${sendAsset} · ${sendAsset === "EVER" ? "Everscale" : "Ethereum"}`)}</label><input id="otc-input" class="amountInput" type="text" inputmode="decimal" autocomplete="off" spellcheck="false" value={sendValue} data-testid={sale ? "otc-offer-ever" : undefined} disabled={busy} on:input={(event) => changeSend(event.currentTarget.value)} on:focus={(event) => event.currentTarget.select()} /><small>{sale ? say("Amount of EVER, not USDT", "Количество EVER, не сумма USDT") : say("Enter the exact amount", "Укажите точную сумму")}</small></div></div>
  <ExchangeFlowBridge reversed={direction === "sell"} onSwap={() => { if (!busy) onDirection(direction === "buy" ? "sell" : "buy"); }} label={`${copy.buy} / ${copy.sell}`} />
  <div class="moneyPanel exchangeMoneyPanel"><div class="panelTop">{@render asset(receiveAsset)}{@render stack(direction === "buy" ? say("You buy", "Покупаете") : say("You receive", "Получаете"), receiveAsset)}</div><div class="panelCopy"><label for="otc-receive">{sale ? direction === "buy" ? say("Total to pay · USDT", "Всего к оплате · USDT") : say("Total to receive · USDT", "Всего к получению · USDT") : say(`You receive · ${receiveAsset} on ${receiveAsset === "EVER" ? "Everscale" : "Ethereum"}`, `Вы получаете · ${receiveAsset} · ${receiveAsset === "EVER" ? "Everscale" : "Ethereum"}`)}</label><input id="otc-receive" class="amountInput amountOutput" readonly={!sale} value={receiveValue} data-testid={sale ? "otc-offer-usdt" : undefined} disabled={busy} inputmode="decimal" autocomplete="off" on:input={(event) => changeReceive(event.currentTarget.value)} placeholder="—" /><small>{sale ? say("Total USDT for the entire offer", "Всего USDT за весь объём EVER") : quote ? say("Fixed net output", "Фиксированная сумма к получению") : say("No quote yet", "Котировки пока нет")}</small></div></div>
    <label class="priceField"><span>{copy.price} · USDT / EVER</span><div><input aria-label={copy.price} readonly={!sale} value={sale ? sale.price : quote ? rate(quote.terms) : "—"} data-testid={sale ? "otc-offer-price" : undefined} disabled={busy} inputmode="decimal" autocomplete="off" spellcheck="false" placeholder="—" on:input={(event) => sale?.onPrice(event.currentTarget.value)} /><span>USDT</span></div></label>
  {#if sale}
    {#if sale.priceRounded}<p class="priceHint">{sale.roundingAsset === "EVER" ? say("The EVER amount is rounded down to 9 decimals. Review the exact amounts before publishing.", "Сумма EVER округлена вниз до 9 знаков. Проверьте точные суммы перед публикацией.") : say("The total is rounded down to 6 USDT decimal places. Review the exact amount before publishing.", "Общая сумма округлена вниз до 6 знаков USDT. Проверьте точную сумму перед публикацией.")}</p>{/if}
    {#if sale.reviewing && sale.seller}<div class="saleReview" data-testid="otc-offer-review"><p class="reviewPending">{say("Not published yet. Check the terms and click Publish offer below.", "Ещё не опубликовано. Проверьте условия и нажмите «Опубликовать предложение» ниже.")}</p><strong>{direction === "buy" ? say("Buy", "Купить") : say("Sell", "Продать")} {offerDecimal(sale.everAmount,9)} EVER · {offerDecimal(sale.usdtAmount,6)} USDT</strong><p>{say("Price", "Курс")}: {sale.askingRate} USDT / EVER</p><p>{direction === "buy" ? say("Your EVER receiving wallet", "Ваш кошелёк для получения EVER") : say("Seller", "Продавец")}: <code>{sale.seller}</code></p><p>{direction === "buy" ? say("USDT paying wallet", "Кошелёк оплаты USDT") : say("USDT recipient", "Получатель USDT")}: <code>{sale.receivingAddress.trim()}</code></p><p>{say("These amounts and addresses will be public for 24 hours, or until you cancel. Publication does not move or reserve funds.", "Эти суммы и адреса будут опубликованы на 24 часа или до отмены. Публикация не переводит и не резервирует средства.")}</p></div>{/if}
  {/if}
  {#if quote}<p class="priceHint">{say("Quote expires", "Котировка истекает")}: {quote.terms.quote_by}</p>{/if}
  {#if status}<p class="priceHint" id="otc-ticket-status" role="status">{status}</p>{/if}
  {#if saleStatus}<p class="priceHint" id="otc-sale-status" role="status">{saleStatus}</p>{/if}
  <button aria-describedby={saleStatus ? "otc-sale-status" : status ? "otc-ticket-status" : undefined} class="cta" type="submit" disabled={busy || (sale ? sale.linkingEvm || (!!sale.seller && !sale.valid) : disabled)} data-testid={sale ? sale.seller ? "otc-publish-offer" : "otc-connect-offer" : "otc-review"}>{sale ? !sale.seller ? say("Connect EVER Wallet to publish", "Подключить EVER Wallet для публикации") : sale.reviewing ? say("Publish offer", "Опубликовать предложение") : say("Review offer", "Проверить предложение") : quote ? say("Review fixed quote", "Проверить котировку") : say("Request exact quote", "Запросить котировку")}<span aria-hidden="true">↗</span></button>
</form>

<style>
  .recipientField { display:grid; gap:8px; margin:16px 0 8px; color:var(--color-text-soft); font-size:12px; }
  .recipientField input { width:100%; min-width:0; min-height:44px; padding:10px 12px; border:1px solid var(--exchange-field-border); border-radius:8px; background:var(--exchange-field-bg); color:var(--color-text); font:inherit; }
  .walletAddress { width:100%; min-height:44px; padding:8px 12px; border:1px solid var(--color-border); border-radius:8px; font-size:12px; }
  .saleReview { margin-bottom:16px; padding:12px; border:1px solid var(--color-border); border-radius:8px; }
  .saleReview .reviewPending { margin:0 0 12px; color:var(--color-text); font-weight:600; }
  .saleReview p { margin-top:8px; color:var(--color-text-soft); font-size:12px; line-height:1.6; }
  .saleReview code { overflow-wrap:anywhere; font-size:11px; }
  .bridgeForm { display: flex; flex: 1; flex-direction: column; min-width: 0; padding: 0 var(--otc-panel-padding, 20px) var(--otc-panel-padding, 20px); }
  .ticketHead { position: relative; display: flex; align-items: center; gap: 8px; padding-top: 4px; }
  .sideTabs { --side-buy: color-mix(in srgb, var(--otc-buy, #247338) 74%, #fff); --side-sell: color-mix(in srgb, var(--otc-sell, #d43d35) 74%, #fff); position: relative; isolation: isolate; display: flex; flex: 1; min-width: 0; gap: 4px; padding: 4px; border-radius: 12px; background: var(--color-panel); }
  .sideTabs::before { position: absolute; z-index: -1; inset: 4px auto 4px 4px; width: calc(50% - 6px); border-radius: 9px; background-image: linear-gradient(90deg, var(--side-buy) 0 34%, #e6d94c 42%, #f0a34d 50%, var(--side-sell) 58% 100%); background-position: 0% 50%; background-size: 300% 100%; content: ""; transition: transform .32s cubic-bezier(.22, 1, .36, 1), background-position .5s ease; }
  .sideTabs.selling::before { background-position: 100% 50%; transform: translateX(calc(100% + 4px)); }
  .settingsWrap { flex: 0 0 auto; }
  .settingsButton { display: grid; width: 40px; min-width: 40px; height: 40px; place-items: center; border: 1px solid var(--exchange-field-border); border-radius: 12px; background: var(--color-panel); transition: background .18s ease, border-color .18s ease; }
  .settingsButton:hover, .settingsButton.active { border-color: var(--color-border-strong); background: var(--color-paper); }
  .settingsButton img { display: block; width: 18px; height: 18px; object-fit: contain; filter: brightness(0); opacity: .62; }
  :global(html[data-theme="dark"]) .settingsButton img { filter: brightness(0) invert(1); }
  .settingsPanel { position: absolute; z-index: 30; top: calc(100% + 8px); right: 0; width: min(320px, 100%); padding: 16px; border: 1px solid var(--color-border-strong); border-radius: 16px; background: var(--color-paper); box-shadow: 0 18px 50px rgb(0 0 0 / .18); }
  .settingsPanelHead { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-bottom: 4px; }
  .settingsPanelHead strong { font-size: 13px; }
  .settingsClose { display: grid; width: 28px; height: 28px; flex: 0 0 auto; place-items: center; border-radius: 7px; color: var(--color-text-soft); font-size: 18px; line-height: 1; }
  .settingsClose:hover { background: var(--color-panel); color: var(--color-text); }
  .sideTabs button { flex: 1; min-width: 0; min-height: 40px; padding: 8px 4px; border-radius: 9px; color: var(--color-text-soft); font-size: 13px; font-weight: 650; line-height: 1.5; transition: color .2s ease; }
  .sideTabs button.active { color: #152016; }
  .quoteMode { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 16px 0; font-size: 12px; line-height: 1.5; }
  .priceField { display: block; color: var(--color-text-soft); font-size: 12px; }
  .priceField > div { display: flex; min-height: 44px; align-items: center; gap: 8px; margin-top: 8px; padding: 8px 12px; border: 1px solid var(--exchange-field-border); border-radius: 8px; background: var(--exchange-field-bg); }
  .priceField input { width: 100%; min-width: 0; min-height: 24px; height: 24px; padding: 0; border: 0; outline: 0; background: transparent; font-family: var(--font-mono); font-size: 16px; }
  .priceField > div > span { flex: 0 0 auto; font-family: var(--font-mono); font-size: 12px; }
  .priceHint { margin: 8px 0 16px; color: var(--color-text-soft); font-size: 12px; line-height: 1.6; overflow-wrap: anywhere; }
  .intentLabel { margin: 0 0 8px; color: var(--color-text-soft); font-family: var(--font-mono); font-size: 12px; font-weight: 400; letter-spacing: .04em; text-transform: uppercase; }
  .intentLabel span::before { display: inline-block; width: 5px; height: 5px; margin-right: 8px; border-radius: 50%; background: var(--color-danger); content: ""; vertical-align: 2px; }
  .receiveLabel span::before { background: var(--color-accent-strong); }
  .moneyPanel { display: grid; flex: 0 0 auto; grid-template-columns: minmax(0, 1fr); min-height: 0; gap: 8px; padding: 12px; border: 1px solid var(--exchange-field-border); border-radius: var(--otc-panel-radius, 16px); background: var(--exchange-field-bg); }
  .panelTop { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; }
  .panelTop > :first-child { flex: 1; min-width: 0; }
  .balanceStack { display: grid; flex: 0 0 auto; max-width: 42%; gap: 1px; justify-items: end; color: var(--color-text-faint); font-size: 11px; line-height: 1.4; text-align: right; }
  .balanceStack b { color: var(--color-text-soft); font-weight: 400; }
  .balanceStack i { color: var(--color-text); font-family: var(--font-mono); font-size: 13px; font-style: normal; overflow-wrap: anywhere; }
  .moneyPanel:hover, .moneyPanel:focus-within { border-width: 1px; }
  .moneyPanel:hover:not(:focus-within) { outline: none; }
  .moneyPanel .panelCopy { display: grid; flex: none; width: 100%; grid-template-rows: 38px 18px; gap: 4px; }
  .moneyPanel .amountInput { min-height: 38px; height: 38px; padding: 0; font-size: 32px; line-height: 1.15; letter-spacing: -.04em; }
  .panelCopy small { overflow: hidden; color: var(--color-text-soft); font-size: 12px; line-height: 18px; text-overflow: ellipsis; white-space: nowrap; }
  .assetIdentity { display: flex; min-width: 0; min-height: 40px; align-items: center; gap: 8px; padding: 8px 10px; border: 1px solid var(--exchange-asset-border); border-radius: 12px; background: var(--exchange-asset-bg); font-size: 13px; line-height: 1.5; }
  .assetIdentity img { flex: 0 0 22px; border-radius: 50%; }
  .assetIdentity strong { font-weight: 650; }
  .assetIdentity span { min-width: 0; margin-left: auto; overflow-wrap: anywhere; color: var(--color-text-soft); font-size: 12px; }
  .cta { display: flex; width: 100%; min-height: 56px; align-items: center; justify-content: space-between; gap: 12px; margin-top: auto; padding: 14px 18px; border: 1px solid var(--color-accent-strong); border-radius: var(--otc-panel-radius, 16px); background: var(--color-accent); color: #152016; font-size: 13px; font-weight: 750; line-height: 1.5; text-align: left; transition: background .18s ease, opacity .18s ease; }
  .cta:hover:not(:disabled) { background: #c9f34b; }
  .cta span { flex: 0 0 auto; font-size: 18px; }
  .cta:disabled { opacity: .45; cursor: default; }
  button:focus-visible, .priceField > div:focus-within, .amountInput:focus-visible { outline: var(--focus-ring-width) solid var(--color-focus); outline-offset: 2px; }
  @media (max-width: 640px) { .moneyPanel .amountInput { font-size: 28px; } .sideTabs button { min-height: 44px; } .balanceStack { max-width: 50%; } }
  @media (prefers-reduced-motion: reduce) { .sideTabs::before, .sideTabs button, .cta { transition: none; } }
</style>
