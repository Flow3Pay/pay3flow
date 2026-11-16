<script lang="ts">
  import { onMount } from "svelte";
  import { locale } from "$lib/i18n";
  import { lockPageScroll } from "$lib/page-scroll-lock";
  import { offerDecimal, p2pFeeSplit, swapPayoutRows } from "$lib/otc/public-offers";
  import { networkIcon, venueIcon } from "$lib/icons";
  import type { Stablecoin } from "$lib/otc/public-offers";
  import { type Direction } from "$lib/otc/service";
  export let direction: Direction = "sell";
  export let stablecoin: Stablecoin = "USDT";
  export let stablecoinNetwork = "ethereum";
  export let swapMinimum = "";
  export let swapFee = "";
  export let swapOptions: Array<{ network: string; minimum: string; fee: string }> = [];
  export let everAmount = "", usdtAmount = "", askingRate = "", seller = "", receivingAddress = "";
  export let error = "";
  export let busy = false;
  export let onClose: () => void;
  export let onPublish: () => void;
  let dialog: HTMLDialogElement;
  $: say = (en: string, ru: string) => $locale === "ru" ? ru : en;
  const networkNames: Record<string, string> = {
    ethereum: "Ethereum", sei: "Sei", "bnb-smart-chain": "BNB Smart Chain", "polygon-pos": "Polygon",
    "avalanche-c": "Avalanche C-Chain", "arbitrum-one": "Arbitrum One", base: "Base", optimism: "Optimism",
    gnosis: "Gnosis", near: "NEAR", tron: "TRON", solana: "Solana", ton: "TON", sui: "Sui", aptos: "Aptos",
  };
  $: networkLabel = networkNames[stablecoinNetwork] ?? stablecoinNetwork;
  $: fee = p2pFeeSplit(usdtAmount);
  $: swapViaIntents = stablecoinNetwork !== "ethereum";
  $: swapTooSmall = swapViaIntents && !!swapMinimum && Number(usdtAmount || 0) > 0 && Number(usdtAmount) < Number(swapMinimum);
  $: payoutRows = swapPayoutRows(usdtAmount, swapOptions);
  onMount(() => {
    const previous = document.activeElement as HTMLElement | null;
    const unlock = lockPageScroll();
    dialog.showModal();
    return () => { dialog.close(); unlock(); previous?.focus(); };
  });
</script>

<dialog bind:this={dialog} class="offerDialog" aria-labelledby="otc-offer-review-title" on:cancel|preventDefault={onClose} on:click={(event) => { if (event.target === dialog) { const rect = dialog.getBoundingClientRect(); if (event.clientX < rect.left || event.clientX > rect.right || event.clientY < rect.top || event.clientY > rect.bottom) onClose(); } }}>
  <div class="offerContent" data-testid="otc-offer-review">
    <div class="dialogTop"><span>{say("Offer preview", "Превью оффера")}</span><button type="button" class="dialogClose" aria-label={say("Close", "Закрыть")} on:click={onClose}><svg width="19" height="19" viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="m6 6 12 12M18 6 6 18" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" /></svg></button></div>
    <h2 id="otc-offer-review-title">{say("Review your offer", "Проверьте предложение")}</h2>
    <p class="reviewPending">{say("Not published yet. Check the terms and press Publish offer.", "Ещё не опубликовано. Проверьте условия и нажмите «Опубликовать предложение».")}</p>
    <strong class="offerSummary">{direction === "buy" ? say("Buy", "Купить") : say("Sell", "Продать")} {offerDecimal(everAmount, 9)} EVER · {offerDecimal(usdtAmount, 6)} {stablecoin}</strong>
    {#if fee}<dl class="feeSplit" data-testid="otc-offer-fee-split"><div><dt>{say("Gross offer amount", "Сумма оффера до комиссии")}</dt><dd>{fee.basis} {stablecoin}</dd></div><div><dt>{say("Service fee (3%)", "Комиссия сервиса (3%)")}</dt><dd>{fee.fee} USDT</dd></div><div class="net"><dt>{say("Seller receives", "Продавец получает")}</dt><dd>{fee.net} {stablecoin}</dd></div></dl>{/if}
    {#if swapViaIntents}
      <p class="swapNote" class:swapWarn={swapTooSmall} data-testid="otc-offer-swap-note">
        <img src={venueIcon("near-intents")} alt="" width="16" height="16" loading="lazy" decoding="async" />
        {#if swapTooSmall}
          <span>{say(`Too small for NEAR Intents · min ${swapMinimum} ${stablecoin}`, `Слишком мало для NEAR Intents · мин. ${swapMinimum} ${stablecoin}`)}</span>
        {:else}
          <span>{say(`Swapped inside by NEAR Intents · ≈${swapFee || "0.3"} USDT fee`, `Обмен внутри через NEAR Intents · фикс ≈${swapFee || "0.3"} USDT`)}</span>
        {/if}
      </p>
    {/if}
    {#if direction === "buy" && payoutRows.length}
      <div class="swapTable" data-testid="otc-swap-table">
        <div class="swapTableHead"><img src={venueIcon("near-intents")} alt="" width="16" height="16" loading="lazy" decoding="async" /><span>{say("Counterparty receives · after NEAR Intents", "Контрагент получит · после NEAR Intents")}</span></div>
        {#each payoutRows as row}
          <div class="swapTableRow">
            <span class="swapTableNet"><img src={networkIcon(networkNames[row.network] ?? row.network)} alt="" width="18" height="18" loading="lazy" decoding="async" />{networkNames[row.network] ?? row.network}</span>
            <strong>{row.net} {stablecoin}</strong>
          </div>
        {/each}
      </div>
    {/if}
    <p>{say("Price", "Курс")}: {askingRate} {stablecoin} / EVER</p>
    <p>{direction === "buy" ? say("Your EVER receiving wallet", "Ваш кошелёк для получения EVER") : say("Seller", "Продавец")}: <code>{seller}</code></p>
    <p>{direction === "buy" ? say(`${stablecoin} paying wallet on ${networkLabel}`, `Кошелёк оплаты ${stablecoin} в сети ${networkLabel}`) : say(`${stablecoin} recipient on ${networkLabel}`, `Получатель ${stablecoin} в сети ${networkLabel}`)}: <code>{receivingAddress.trim()}</code></p>
    <p>{say("The 3% service fee is denominated in USDT. For a USDC offer it is calculated at nominal 1:1 parity, not a live exchange rate. The seller receives the offer amount minus the fee; the designated payer separately signs the fee transfer. Any route cost is shown separately before signing.", "Комиссия 3% учитывается в USDT. Для оффера в USDC она рассчитывается по номинальному курсу 1:1, а не по биржевому курсу. Продавец получает сумму оффера за вычетом комиссии; назначенный плательщик отдельно подтверждает перевод комиссии. Расходы маршрута показываются отдельно до подписи.")}</p>
    <p>{say("These amounts and addresses will be public for 24 hours, or until you cancel. Publication does not move or reserve funds.", "Эти суммы и адреса будут опубликованы на 24 часа или до отмены. Публикация не переводит и не резервирует средства.")}</p>
    {#if error}<p class="reviewError" role="alert">{error}</p>{/if}
    <button type="button" class="confirm" data-testid="otc-offer-publish" disabled={busy} on:click={onPublish}>{say("Publish offer", "Опубликовать предложение")}<span aria-hidden="true">↗</span></button>
    <button type="button" class="back" on:click={onClose}>{say("Back to editing", "Вернуться к редактированию")}</button>
  </div>
</dialog>

<style>
  .offerDialog { position: fixed; width: min(430px, calc(100% - 32px)); max-height: calc(100dvh - 32px); padding: 0; margin: auto; overflow-y: auto; border: 1px solid var(--color-border); border-radius: 18px; background: var(--color-paper); color: var(--color-text); box-shadow: 0 24px 120px #0004; }
  .offerDialog::backdrop { background: #0a100b99; backdrop-filter: blur(5px); }
  .offerContent { padding: 24px; }
  .dialogTop { display: flex; align-items: center; justify-content: space-between; }
  .dialogTop > span { padding: 5px 8px; border: 1px solid var(--color-border); border-radius: 6px; font-family: var(--font-mono); font-size: 12px; color: var(--color-text-soft); }
  .dialogClose { display: grid; width: 32px; height: 32px; place-items: center; border-radius: 7px; background: var(--color-panel); color: var(--color-text-soft); }
  .dialogClose:hover { color: var(--color-text); }
  h2 { margin-top: 20px; font-size: 25px; letter-spacing: -.05em; }
  .reviewPending { margin: 7px 0 18px; color: var(--color-text); font-size: 12px; font-weight: 600; line-height: 1.6; }
  .offerSummary { display: block; padding: 16px; border-radius: 12px; background: var(--color-panel); font-size: 14px; }
  .offerDialog p { margin-top: 8px; color: var(--color-text-soft); font-size: 12px; line-height: 1.6; }
  .offerDialog code { overflow-wrap: anywhere; font-size: 11px; }
  .feeSplit { display: grid; gap: 6px; margin: 12px 0 0; padding: 12px 0 0; border-top: 1px solid var(--color-border); }
  .feeSplit > div { display: flex; align-items: baseline; justify-content: space-between; gap: 12px; }
  .feeSplit dt { color: var(--color-text-soft); font-size: 11px; }
  .feeSplit dd { margin: 0; font-family: var(--font-mono); font-size: 11px; }
  .feeSplit .net dt, .feeSplit .net dd { color: var(--color-text); font-weight: 650; }
  .swapTable { display: grid; gap: 2px; margin: 12px 0 0; padding: 10px 12px; border: 1px solid var(--color-border); border-radius: 10px; }
  .swapTableHead { display: flex; align-items: center; gap: 7px; padding-bottom: 6px; border-bottom: 1px solid var(--color-border); color: var(--color-text-soft); font-size: 11px; line-height: 1.4; }
  .swapTableHead img { flex: 0 0 auto; border-radius: 4px; }
  .swapTableRow { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 5px 0; font-size: 12px; }
  .swapTableNet { display: inline-flex; align-items: center; gap: 7px; min-width: 0; color: var(--color-text-soft); }
  .swapTableNet img { flex: 0 0 auto; border-radius: 50%; }
  .swapTableRow strong { font-family: var(--font-mono); font-size: 11px; white-space: nowrap; }
  .swapNote { display: flex; align-items: center; gap: 8px; padding: 8px 10px; border: 1px solid var(--color-border); border-radius: 7px; }
  .swapNote img { flex: 0 0 auto; border-radius: 4px; }
  .swapNote.swapWarn { border-color: var(--color-danger); color: var(--color-danger); }
  .reviewError { margin-top: 14px; padding: 10px 12px; border: 1px solid var(--color-danger); border-radius: 9px; color: var(--color-danger); font-size: 12px; line-height: 1.6; }
  .confirm { display: flex; justify-content: space-between; gap: 12px; width: 100%; margin-top: 18px; padding: 15px; border-radius: 9px; background: var(--color-accent); color: #152016; font-size: 12px; font-weight: 800; }
  .confirm:hover:not(:disabled) { background: var(--color-accent-strong); }
  .confirm:disabled { opacity: .45; cursor: default; }
  .back { display: block; width: 100%; margin-top: 9px; padding: 10px; color: var(--color-text-soft); font-size: 12px; }
</style>
