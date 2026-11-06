<script lang="ts">
  import { tick } from "svelte";
  import ExchangeFlowBridge from "../ExchangeFlowBridge.svelte";
  import PaymentMethodPicker from "../PaymentMethodPicker.svelte";
  import OtcOfferReview from "./OtcOfferReview.svelte";
  import { locale } from "$lib/i18n";
  import { networkIcon } from "$lib/icons";
  import type { CryptoNetwork } from "$lib/networks";
  import { otcCopy } from "$lib/otc/copy";
  import type { PaymentMethod } from "$lib/payment-methods";
  import { rate, type Direction, type Trade } from "$lib/otc/service";
  import { type SaleTicketState, type Stablecoin } from "$lib/otc/public-offers";
  import { everFromUsd, offerTotal } from "$lib/otc/offer-amount";
  import { formatTokenMarketValue, formatUsdAmount, formatUsdMarketValue, sanitizeDecimalInput } from "$lib/usd-amount";
  export let sale: SaleTicketState | null = null;
  export let direction: Direction = "buy";
  export let input = "100";
  export let quote: Trade | null = null;
  export let disabled = false, busy = false;
  export let status = "";
  export let onDirection: (direction: Direction) => void;
  export let onEdit: () => void;
  export let onRequest: () => void;
  export let onReview: () => void;
  $: copy = otcCopy($locale);
  $: say = (en: string, ru: string) => $locale === "ru" ? ru : en;
  $: sendAsset = sale ? "EVER" : direction === "buy" ? "USDT" : "EVER";
  $: receiveAsset = sale ? sale.stablecoin : direction === "buy" ? "EVER" : "USDT";
  $: sendValue = sale ? sale.everAmount : input;
  $: receiveValue = sale ? sale.usdtAmount : quote?.terms.output || "";
  let usdMode = false, usdEntry = "", sendFocused = false;
  $: usdRate = Number(sale?.marketUsdRate || 0);
  $: usdAvailable = !!sale && Number.isFinite(usdRate) && usdRate > 0;
  $: if (usdMode && !usdAvailable) { usdMode = false; usdEntry = ""; }
  $: usdValue = sale && usdAvailable ? offerTotal(sale.everAmount, sale.marketUsdRate)?.amount || "" : "";
  $: displayedSendValue = sale && usdMode && usdAvailable
    ? sendFocused && usdEntry ? usdEntry : formatUsdAmount(Number(usdValue))
    : sendValue;
  $: sendShort = sale?.shortfall === "ever";
  $: receiveShort = sale?.shortfall === "usdt";
  $: shortfallNote = (asset: "EVER" | "USDT" | "USDC") => {
    const available = sale?.fundedBalance ? say(` — available ${sale.fundedBalance} ${asset}`, ` — доступно ${sale.fundedBalance} ${asset}`) : "";
    return say(`Not enough ${asset} in your wallet${available}. You can still publish the offer.`, `Недостаточно ${asset} на кошельке${available}. Опубликовать предложение всё равно можно.`);
  };
  const stablecoinMethods: PaymentMethod[] = [
    { id: "otc-usdt", name: "Tether", country: "global", currency: "USDT", role: "recipient", kind: "wallet", color: "#26a17b", initials: "USDT", iconUrl: "/icons/assets/usdt.png", p2pQuery: "USDT" },
    { id: "otc-usdc", name: "USD Coin", country: "global", currency: "USDC", role: "recipient", kind: "wallet", color: "#2775ca", initials: "USDC", iconUrl: "/icons/assets/usdc.png", p2pQuery: "USDC" },
  ];
  const networkNames: Record<string, string> = {
    ethereum: "Ethereum (ERC-20)",
    "avalanche-c": "Avalanche C-Chain",
    "polygon-pos": "Polygon PoS",
    "arbitrum-one": "Arbitrum One",
    "bnb-smart-chain": "BNB Smart Chain",
    near: "NEAR",
    solana: "Solana",
    ton: "TON",
    tron: "TRON",
    aptos: "Aptos",
    base: "Base",
    optimism: "Optimism",
    sei: "Sei",
  };
  function makeStablecoinNetworks(assets: SaleTicketState["supportedStablecoinAssets"] = []): CryptoNetwork[] {
    const networks = new Map<string, CryptoNetwork>([["ethereum", {
      id: "ethereum",
      name: networkNames.ethereum,
      currencies: ["USDT", "USDC"],
    }]]);
    for (const asset of assets) {
      const network = networks.get(asset.network) ?? {
        id: asset.network,
        name: networkNames[asset.network] ?? asset.network,
        currencies: [],
      };
      if (!network.currencies.includes(asset.symbol)) network.currencies.push(asset.symbol);
      networks.set(asset.network, network);
    }
    return [...networks.values()];
  }
  let stablecoinPickerOpen = false;
  $: stablecoinPickerNetworks = makeStablecoinNetworks(sale?.supportedStablecoinAssets);
  $: selectedStablecoinMethod = stablecoinMethods.find((method) => method.currency === sale?.stablecoin) ?? null;
  $: selectedStablecoinNetwork = stablecoinPickerNetworks.find((network) => network.id === sale?.stablecoinNetwork);
  function selectStablecoin(method: PaymentMethod, network?: CryptoNetwork) {
    if (!sale || !network || (method.currency !== "USDT" && method.currency !== "USDC")) return;
    const token: Stablecoin = method.currency;
    if (sale.stablecoin !== token) sale.onStablecoin(token);
    if (sale.stablecoinNetwork !== network.id) sale.onStablecoinNetwork(network.id);
    stablecoinPickerOpen = false;
  }
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
    if (sale && usdMode && usdAvailable) {
      const nextUsd = sanitizeDecimalInput(value);
      usdEntry = nextUsd;
      sale.onEver(everFromUsd(nextUsd, sale.marketUsdRate)?.amount || "");
      return;
    }
    if (sale) sale.onEver(value);
    else { input=value; onEdit(); }
  }
  function focusSend(event: FocusEvent) {
    sendFocused = true;
    (event.currentTarget as HTMLInputElement).select();
  }
  function blurSend() { sendFocused = false; usdEntry = ""; }
  function toggleUsd() {
    if (!usdAvailable) return;
    usdMode = !usdMode;
    usdEntry = "";
    void tick().then(() => document.getElementById("otc-input")?.focus());
  }
  $: maxBalance = Number(sale?.fundedBalance || 0) > 0 ? sale?.fundedBalance || "" : "";
  $: maxActive = !!maxBalance && Number(direction === "sell" ? sale?.everAmount : sale?.usdtAmount) === Number(maxBalance);
  function applyMax() {
    if (!sale || !maxBalance) return;
    usdEntry = "";
    const value = maxActive ? "" : maxBalance;
    if (direction === "sell") sale.onEver(value);
    else sale.onUsdt(value);
  }
  function changeReceive(value: string) { sale?.onUsdt(value); }

</script>
{#snippet asset(asset: string)}
  {@const networkName = asset === "EVER" ? "Everscale" : "Ethereum"}
  <div class="assetIdentity">
    <span class="assetMain"><span class="assetAvatar" aria-hidden="true"><img src={asset === "EVER" ? "/icons/assets/ever.svg" : "/icons/assets/usdt.png"} alt="" width="28" height="28" /></span><span class="assetText"><strong>{asset}</strong></span></span>
    <span class="assetNetwork"><span class="assetDot" aria-hidden="true"><img src={networkIcon(networkName)} alt="" width="18" height="18" /></span><span class="assetCopy"><strong>{networkName}</strong></span></span>
  </div>
{/snippet}
{#snippet shortfallMark(asset: "EVER" | "USDT" | "USDC")}
  <button type="button" class="shortfallMark" data-testid="otc-shortfall-warning" data-tooltip={shortfallNote(asset)} aria-label={shortfallNote(asset)}>
    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <path d="M12 9V14" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
      <path d="M12.0001 21.4093H5.94005C2.47005 21.4093 1.02005 18.9293 2.70005 15.8993L5.82006 10.2793L8.76006 4.9993C10.5401 1.7893 13.4601 1.7893 15.2401 4.9993L18.1801 10.2893L21.3001 15.9093C22.9801 18.9393 21.5201 21.4193 18.0601 21.4193H12.0001V21.4093Z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
      <path d="M11.9945 17H12.0035" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
    </svg>
  </button>
{/snippet}
<svelte:window on:pointerdown={outsideSettings} on:keydown={settingsKeys} />
<form class="bridgeForm" on:submit|preventDefault={() => sale ? sale.onRequest() : quote ? onReview() : onRequest()}>
  <div class="ticketHead">
    <div class="sideTabs" class:selling={direction === "sell"} role="group" aria-label="Trade direction"><button type="button" disabled={busy} class:active={direction === "buy"} aria-pressed={direction === "buy"} on:click={() => { closeSettings(); onDirection("buy"); }}>{say("Buy EVER", "Купить EVER")}</button><button type="button" disabled={busy} class:active={direction === "sell"} aria-pressed={direction === "sell"} on:click={() => { closeSettings(); onDirection("sell"); }}>{say("Sell EVER", "Продать EVER")}</button></div>
    {#if sale}
      <div class="settingsWrap" bind:this={settingsWrap}>
        <button type="button" class="settingsButton" class:active={settingsOpen} aria-haspopup="dialog" aria-expanded={settingsOpen} aria-label={say("Offer settings", "Настройки предложения")} title={say("Offer settings", "Настройки предложения")} on:click={() => settingsOpen ? closeSettings() : openSettings()}><img src="/icons/ui/route-settings.svg" alt="" width="18" height="18" /></button>
        {#if settingsOpen}
          <div class="settingsPanel" role="dialog" aria-label={say("Offer settings", "Настройки предложения")} tabindex="-1">
            <div class="settingsPanelHead"><strong>{say("Offer settings", "Настройки предложения")}</strong><button type="button" class="settingsClose" aria-label={say("Close settings", "Закрыть настройки")} on:click={() => closeSettings(true)}>×</button></div>
            <label class="recipientField">{say(`Your ${sale.stablecoin} ${direction === "buy" ? "paying" : "receiving"} address on ${sale.stablecoinNetwork}`, `Ваш ${direction === "buy" ? "адрес оплаты" : "адрес получения"} ${sale.stablecoin} в сети ${sale.stablecoinNetwork}`)}<input bind:this={settingsInput} data-testid="otc-offer-recipient" value={sale.receivingAddress} on:input={(event) => sale?.onAddress(event.currentTarget.value)} autocomplete="off" spellcheck="false" placeholder={sale.stablecoinNetwork === "ethereum" ? "0x…" : say("Wallet address", "Адрес кошелька")} disabled={busy} /></label>
            {#if sale.stablecoinNetwork === "ethereum"}<button class="walletAddress" type="button" disabled={busy || sale.linkingEvm} on:click={sale.onUseEthereum}>{sale.linkingEvm ? say("Connecting…", "Подключаем…") : say("Use my Ethereum wallet address", "Использовать адрес моего Ethereum-кошелька")}</button>{/if}
          </div>
        {/if}
      </div>
    {/if}
  </div>
  <div class="moneyStack">
  <div class="moneyPanel exchangeMoneyPanel"><div class="panelTop">{@render asset(sendAsset)}</div><div class="panelCopy"><label for="otc-input">{sale ? direction === "buy" ? say(usdMode ? "Buy amount · USD" : "Buy amount · EVER", usdMode ? "Количество к покупке · USD" : "Количество к покупке · EVER") : say(usdMode ? "Sell amount · USD" : "Sell amount · EVER", usdMode ? "Количество к продаже · USD" : "Количество к продаже · EVER") : say(`You send · ${sendAsset} on ${sendAsset === "EVER" ? "Everscale" : "Ethereum"}`, `Вы отправляете · ${sendAsset} · ${sendAsset === "EVER" ? "Everscale" : "Ethereum"}`)}</label><div class="amountRow"><input id="otc-input" class="amountInput" class:shortfall={sendShort} type="text" inputmode="decimal" autocomplete="off" spellcheck="false" value={displayedSendValue} data-testid={sale ? "otc-offer-ever" : undefined} disabled={busy} on:input={(event) => changeSend(event.currentTarget.value)} on:focus={focusSend} on:blur={blurSend} />{#if sendShort}{@render shortfallMark("EVER")}{/if}</div>{#if sale}<div class="marketMeta"><span class="marketValue" data-testid="otc-market-value" aria-label={usdMode && usdAvailable ? say("Approximate EVER amount", "Примерная сумма в EVER") : say("Approximate USD market value", "Примерная рыночная стоимость в долларах")}>{usdMode && usdAvailable ? formatTokenMarketValue(Number(sale.everAmount), "EVER") : formatUsdMarketValue(Number(usdValue))}</span><button type="button" class="usdToggle" class:usdToggleActive={usdMode} aria-pressed={usdMode} disabled={!usdAvailable || busy} title={say("Switch amount between USD and EVER", "Переключить сумму между USD и EVER")} aria-label={say("Switch amount between USD and EVER", "Переключить сумму между USD и EVER")} on:click={toggleUsd}>USD</button>{#if maxBalance}<button type="button" class="usdToggle" class:usdToggleActive={maxActive} data-testid="otc-amount-max" aria-pressed={maxActive} disabled={busy} title={maxActive ? say("Clear the amount", "Сбросить сумму") : say("Use the whole available balance", "Внести весь доступный баланс")} aria-label={maxActive ? say("Clear the amount", "Сбросить сумму") : say("Use the whole available balance", "Внести весь доступный баланс")} on:click={applyMax}>{say("Max", "Макс")}</button>{/if}</div>{/if}</div></div>
  <ExchangeFlowBridge overlay reversed={direction === "sell"} onSwap={() => { if (!busy) onDirection(direction === "buy" ? "sell" : "buy"); }} label={`${copy.buy} / ${copy.sell}`} />
  <div class="moneyPanel exchangeMoneyPanel"><div class="panelTop">{#if sale}<button type="button" class="stablecoinNetworkPicker" aria-haspopup="dialog" aria-label={say(`Choose token and network, currently ${sale.stablecoin} on ${selectedStablecoinNetwork?.name ?? sale.stablecoinNetwork}`, `Выбрать токен и сеть, сейчас ${sale.stablecoin} в сети ${selectedStablecoinNetwork?.name ?? sale.stablecoinNetwork}`)} on:click={() => stablecoinPickerOpen = true}><span class="stablecoinNetworkMain"><img src={sale.stablecoin === "USDC" ? "/icons/assets/usdc.png" : "/icons/assets/usdt.png"} alt="" width="28" height="28" /><span><strong>{sale.stablecoin}</strong><small>{selectedStablecoinNetwork?.name ?? sale.stablecoinNetwork}</small></span></span><span class="stablecoinNetworkSide"><span class="assetDot" aria-hidden="true"><img src={networkIcon(selectedStablecoinNetwork?.name ?? sale.stablecoinNetwork)} alt="" width="18" height="18" /></span><svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="m4 6 4 4 4-4" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" /></svg></span></button>{:else}{@render asset(receiveAsset)}{/if}</div><div class="panelCopy"><label for="otc-receive">{sale ? direction === "buy" ? say(`Total to pay · ${sale.stablecoin}`, `Всего к оплате · ${sale.stablecoin}`) : say(`Total to receive · ${sale.stablecoin}`, `Всего к получению · ${sale.stablecoin}`) : say(`You receive · ${receiveAsset} on ${receiveAsset === "EVER" ? "Everscale" : "Ethereum"}`, `Вы получаете · ${receiveAsset} · ${receiveAsset === "EVER" ? "Everscale" : "Ethereum"}`)}</label><div class="amountRow"><input id="otc-receive" class="amountInput amountOutput" class:shortfall={receiveShort} readonly={!sale} value={receiveValue} data-testid={sale ? "otc-offer-usdt" : undefined} disabled={busy} inputmode="decimal" autocomplete="off" on:input={(event) => changeReceive(event.currentTarget.value)} placeholder={sale?.marketApprox ? `≈ ${sale.marketApprox}` : "—"} />{#if receiveShort}{@render shortfallMark(sale?.stablecoin || "USDT")}{/if}</div></div></div>
  </div>
  <div class="priceField">
    <div class="priceHead">
      <span>{copy.price} · {sale?.stablecoin || "USDT"} / EVER</span>
      {#if sale}
        <label class="marketToggle">
          <input type="checkbox" checked={sale.marketPrice} disabled={busy || !sale.marketRate} on:change={(event) => sale.onMarketPrice(event.currentTarget.checked)} />
          <span>{say("Market price", "Рыночная цена")}</span>
        </label>
      {/if}
    </div>
    <div class="priceFieldRow"><input aria-label={copy.price} readonly={!sale || sale.marketPrice} value={sale ? sale.price : quote ? rate(quote.terms) : "—"} data-testid={sale ? "otc-offer-price" : undefined} disabled={busy} inputmode="decimal" autocomplete="off" spellcheck="false" placeholder="—" on:input={(event) => sale?.onPrice(event.currentTarget.value)} /><span>{sale?.stablecoin || "USDT"}</span></div>
  </div>
  {#if sale}
    {#if sale.priceRounded}<p class="priceHint">{sale.roundingAsset === "EVER" ? say("The EVER amount is rounded down to 9 decimals. Review the exact amounts before publishing.", "Сумма EVER округлена вниз до 9 знаков. Проверьте точные суммы перед публикацией.") : say(`The total is rounded down to 6 ${sale.stablecoin} decimal places. Review the exact amount before publishing.`, `Общая сумма округлена вниз до 6 знаков ${sale.stablecoin}. Проверьте точную сумму перед публикацией.`)}</p>{/if}
    {#if sale.amountFloored}<p class="priceHint">{sale.amountFloored === "EVER" ? say("EVER has 9 decimals, so the extra digits you typed were dropped. Review the exact amount before publishing.", "У EVER 9 знаков после запятой, лишние введённые цифры отброшены. Проверьте точную сумму перед публикацией.") : say(`${sale.stablecoin} has 6 decimals, so the extra digits you typed were dropped. Review the exact amount before publishing.`, `У ${sale.stablecoin} 6 знаков после запятой, лишние введённые цифры отброшены. Проверьте точную сумму перед публикацией.`)}</p>{/if}
    {#if sale.reviewing && sale.seller}<OtcOfferReview {direction} stablecoin={sale.stablecoin} stablecoinNetwork={sale.stablecoinNetwork} everAmount={sale.everAmount} usdtAmount={sale.usdtAmount} askingRate={sale.askingRate} seller={sale.seller} receivingAddress={sale.receivingAddress} error={sale.error} busy={busy} onClose={sale.onCloseReview} onPublish={() => sale.onRequest()} />{/if}
  {/if}
  {#if quote}<p class="priceHint">{say("Quote expires", "Котировка истекает")}: {quote.terms.quote_by}</p>{/if}
  {#if status}<p class="priceHint" id="otc-ticket-status" role="status">{status}</p>{/if}
  <button aria-describedby={status ? "otc-ticket-status" : undefined} class="cta" type="submit" disabled={busy || (sale ? sale.linkingEvm || (!!sale.seller && !sale.valid) : disabled)} data-testid={sale ? sale.seller ? "otc-publish-offer" : "otc-connect-offer" : "otc-review"}>{sale ? !sale.seller ? say("Connect EVER Wallet to publish", "Подключить EVER Wallet для публикации") : sale.reviewing ? say("Publish offer", "Опубликовать предложение") : say("Review offer", "Проверить предложение") : quote ? say("Review fixed quote", "Проверить котировку") : say("Request exact quote", "Запросить котировку")}<span aria-hidden="true">↗</span></button>
</form>
{#if sale}<PaymentMethodPicker open={stablecoinPickerOpen} title={say("Choose token and network", "Выберите токен и сеть")} role="recipient" networks={stablecoinPickerNetworks} paymentMethods={stablecoinMethods} selected={selectedStablecoinMethod} selectedNetwork={selectedStablecoinNetwork} onClose={() => stablecoinPickerOpen = false} onSelect={selectStablecoin} />{/if}

<style>
  .recipientField { display:grid; gap:8px; margin:16px 0 8px; color:var(--color-text-soft); font-size:12px; }
  .recipientField input { width:100%; min-width:0; min-height:44px; padding:10px 12px; border:1px solid var(--exchange-field-border); border-radius:8px; background:var(--exchange-field-bg); color:var(--color-text); font:inherit; }
  .walletAddress { width:100%; min-height:44px; padding:8px 12px; border:1px solid var(--color-border); border-radius:8px; font-size:12px; }
  .bridgeForm { display: flex; flex: 1; flex-direction: column; min-width: 0; padding: 0 var(--otc-panel-padding, 20px) 16px; }
  .ticketHead { position: relative; display: flex; align-items: center; gap: 8px; margin-bottom: 12px; padding-top: 2px; }
  .sideTabs { --side-buy: var(--otc-buy, var(--color-accent-strong)); --side-sell: var(--otc-sell, var(--color-danger)); position: relative; isolation: isolate; display: flex; flex: 1; min-width: 0; gap: 4px; padding: 4px; border-radius: 12px; background: var(--color-panel); }
  .sideTabs::before { position: absolute; z-index: -1; inset: 4px auto 4px 4px; width: calc(50% - 6px); border-radius: 9px; background-image: linear-gradient(90deg, var(--side-buy) 0 40%, #e6d94c 46%, #f0a34d 52%, var(--side-sell) 58% 100%); background-position: 0% 50%; background-size: 300% 100%; content: ""; transition: transform .32s cubic-bezier(.22, 1, .36, 1), background-position .5s ease; }
  .sideTabs.selling::before { background-position: 100% 50%; transform: translateX(calc(100% + 4px)); }
  .settingsWrap { flex: 0 0 auto; }
  .settingsButton { display: grid; width: 40px; min-width: 40px; height: 40px; place-items: center; border: 1px solid var(--exchange-field-border); border-radius: 12px; background: var(--color-panel); transition: background .18s ease, border-color .18s ease; }
  .settingsButton:hover, .settingsButton.active { border-color: var(--color-border-strong); background: var(--color-paper); }
  .settingsButton img { display: block; width: 18px; height: 18px; object-fit: contain; filter: brightness(0); opacity: .62; }
  :global(html[data-theme="dark"]) .settingsButton img { filter: brightness(0) invert(1); }
  .settingsPanel { position: absolute; z-index: 30; top: calc(100% + 8px); right: 0; width: min(320px, 100%); padding: 16px; border: 1px solid var(--color-border-strong); border-radius: var(--radius-panel, 12px); background: var(--color-paper); box-shadow: 0 18px 50px rgb(0 0 0 / .18); }
  .settingsPanelHead { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-bottom: 4px; }
  .settingsPanelHead strong { font-size: 13px; }
  .settingsClose { display: grid; width: 28px; height: 28px; flex: 0 0 auto; place-items: center; border-radius: 7px; color: var(--color-text-soft); font-size: 18px; line-height: 1; }
  .settingsClose:hover { background: var(--color-panel); color: var(--color-text); }
  .sideTabs button { flex: 1; min-width: 0; min-height: 40px; padding: 8px 4px; border-radius: 9px; color: var(--color-text-soft); font-size: 13px; font-weight: 650; line-height: 1.5; transition: color .2s ease; }
  .sideTabs button.active { color: #152016; }
  .sideTabs.selling button.active { color: var(--color-text-invert); }
  .priceField { display: block; margin-top: 6px; color: var(--color-text-soft); font-size: 12px; }
  .priceHead { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  .marketToggle { display: inline-flex; align-items: center; gap: 6px; cursor: pointer; color: var(--color-text-soft); font-size: 12px; line-height: 1.4; white-space: nowrap; }
  .marketToggle input { width: 16px; height: 16px; margin: 0; accent-color: var(--color-accent-strong); }
  .marketToggle input:disabled { cursor: default; }
  .marketToggle input:disabled + span { opacity: .5; }
  .priceFieldRow { display: flex; min-height: 38px; align-items: center; gap: 8px; margin-top: 6px; padding: 6px 10px; border: 1px solid var(--exchange-field-border); border-radius: 7px; background: var(--exchange-field-bg); }
  .priceFieldRow input { width: 100%; min-width: 0; min-height: 24px; height: 24px; padding: 0; border: 0; outline: 0; background: transparent; font-family: var(--font-mono); font-size: 16px; }
  .priceFieldRow > span { flex: 0 0 auto; font-family: var(--font-mono); font-size: 12px; }
  .moneyPanel .amountOutput::placeholder { color: var(--color-text-soft); font-size: 20px; letter-spacing: -.02em; opacity: .55; }
  .priceHint { margin: 6px 0 12px; color: var(--color-text-soft); font-size: 12px; line-height: 1.6; overflow-wrap: anywhere; }
  .intentLabel { margin: 0 0 8px; color: var(--color-text-soft); font-family: var(--font-mono); font-size: 12px; font-weight: 400; letter-spacing: .04em; text-transform: uppercase; }
  .intentLabel span::before { display: inline-block; width: 5px; height: 5px; margin-right: 8px; border-radius: 50%; background: var(--color-danger); content: ""; vertical-align: 2px; }
  .receiveLabel span::before { background: var(--color-accent-strong); }
  .moneyStack { position: relative; display: flex; flex-direction: column; gap: 6px; }
  .moneyPanel { display: grid; flex: 0 0 auto; grid-template-columns: minmax(0, 1fr); min-height: 0; gap: 6px; padding: 10px; border: 1px solid var(--exchange-field-border); border-radius: 7px; background: var(--exchange-field-bg); }
  .panelTop { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; }
  .panelTop > :first-child { flex: 1; min-width: 0; }
  .moneyPanel:focus-within { border-width: 1px; border-color: var(--exchange-field-border); outline: none; }
  .moneyPanel .panelCopy { display: grid; flex: none; width: 100%; grid-template-rows: 34px; gap: 2px; }
  .moneyPanel .amountInput { min-height: 34px; height: 34px; padding: 0; font-size: 28px; line-height: 1.15; letter-spacing: -.04em; }
  .moneyPanel .amountRow { display: flex; min-width: 0; align-items: center; gap: 6px; }
  .moneyPanel .amountRow .amountInput { width: auto; flex: 1 1 auto; min-width: 0; }
  .moneyPanel .amountInput.shortfall { color: var(--color-warn); }
  .shortfallMark { display: grid; width: 20px; height: 20px; flex: 0 0 auto; place-items: center; border: 0; padding: 0; background: transparent; color: var(--color-warn); }
  .assetIdentity { display: flex; min-width: 0; align-items: center; justify-content: flex-end; overflow: hidden; border: 1px solid var(--exchange-asset-border); border-radius: 5px; background: var(--exchange-asset-bg); }
  .assetMain { display: flex; min-width: 0; flex: 1 1 auto; align-items: center; gap: 7px; min-height: 40px; padding: 5px 7px 5px 6px; }
  .assetAvatar { display: grid; flex: 0 0 auto; width: 28px; height: 28px; place-items: center; overflow: hidden; border-radius: 50%; }
  .assetAvatar img { width: 20px; height: 20px; border-radius: 50%; object-fit: contain; }
  .assetText { min-width: 0; }
  .assetText strong { color: var(--color-text); font-size: 14px; font-weight: 650; line-height: 1.2; }
  .assetNetwork { display: flex; flex: 0 0 auto; align-items: center; gap: 7px; min-height: 40px; padding: 5px 7px; border-left: 1px solid var(--exchange-asset-border); }
  .assetDot { display: grid; flex: 0 0 auto; width: 18px; height: 18px; place-items: center; overflow: hidden; border-radius: 50%; }
  .assetDot img { display: block; width: 100%; height: 100%; object-fit: contain; }
  .assetCopy { min-width: 0; }
  .assetCopy strong { color: #344235; font-size: 12px; font-weight: 700; line-height: 1; }
  :global(html[data-theme="dark"]) .assetCopy strong { color: var(--color-text); }
  .stablecoinNetworkPicker { display:flex; width:100%; min-height:44px; align-items:center; justify-content:space-between; gap:10px; padding:0 2px; text-align:left; }
  .stablecoinNetworkMain,.stablecoinNetworkSide { display:flex; min-width:0; align-items:center; gap:8px; }
  .stablecoinNetworkMain > img { flex:0 0 auto; border-radius:50%; }
  .stablecoinNetworkMain > span { display:grid; min-width:0; gap:2px; }
  .stablecoinNetworkMain strong { color:var(--color-text); font-size:14px; }
  .stablecoinNetworkMain small { overflow:hidden; color:var(--color-text-faint); font-size:10px; text-overflow:ellipsis; white-space:nowrap; }
  .stablecoinNetworkSide { flex:0 0 auto; color:var(--color-text-faint); }
  .stablecoinNetworkPicker:focus-visible { outline:var(--focus-ring-width) solid var(--color-focus); outline-offset:2px; }
  .cta { display: flex; width: 100%; min-height: 50px; align-items: center; justify-content: space-between; gap: 12px; margin-top: auto; padding: 12px 16px; border: 1px solid var(--color-accent-strong); border-radius: 5px; background: var(--color-accent); color: #152016; font-size: 13px; font-weight: 750; line-height: 1.5; text-align: left; transition: background .18s ease, opacity .18s ease; }
  .cta:hover:not(:disabled) { background: color-mix(in srgb, var(--color-accent) 82%, #fff); }
  .cta span { flex: 0 0 auto; font-size: 18px; }
  .cta:disabled { opacity: .45; cursor: default; }
  button:focus-visible, .priceFieldRow:focus-within { outline: var(--focus-ring-width) solid var(--color-focus); outline-offset: 2px; }
  @media (max-width: 640px) { .moneyPanel .amountInput { font-size: 28px; } .sideTabs button { min-height: 44px; } }
  @media (prefers-reduced-motion: reduce) { .sideTabs::before, .sideTabs button, .cta { transition: none; } }
</style>
