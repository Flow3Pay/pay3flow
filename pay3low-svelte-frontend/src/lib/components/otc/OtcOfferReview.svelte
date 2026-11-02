<script lang="ts">
  import { onMount } from "svelte";
  import { locale } from "$lib/i18n";
  import { lockPageScroll } from "$lib/page-scroll-lock";
  import { offerDecimal, p2pFeeSplit } from "$lib/otc/public-offers";
  import { type Direction } from "$lib/otc/service";
  export let direction: Direction = "sell";
  export let everAmount = "", usdtAmount = "", askingRate = "", seller = "", receivingAddress = "";
  export let error = "";
  export let busy = false;
  export let onClose: () => void;
  export let onPublish: () => void;
  let dialog: HTMLDialogElement;
  $: say = (en: string, ru: string) => $locale === "ru" ? ru : en;
  $: fee = p2pFeeSplit(usdtAmount);
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
    <strong class="offerSummary">{direction === "buy" ? say("Buy", "Купить") : say("Sell", "Продать")} {offerDecimal(everAmount, 9)} EVER · {offerDecimal(usdtAmount, 6)} USDT</strong>
    {#if fee}<dl class="feeSplit" data-testid="otc-offer-fee-split"><div><dt>{say("Amount", "Сумма")}</dt><dd>{fee.basis} USDT</dd></div><div><dt>{say("Service fee (3%)", "Комиссия сервиса (3%)")}</dt><dd>−{fee.fee} USDT</dd></div><div><dt>{say("Amount to exchange", "Сумма для обмена")}</dt><dd>{fee.net} USDT</dd></div><div class="net"><dt>{say("You receive", "К получению")}</dt><dd>{fee.net} USDT</dd></div></dl>{/if}
    <p>{say("Price", "Курс")}: {askingRate} USDT / EVER</p>
    <p>{direction === "buy" ? say("Your EVER receiving wallet", "Ваш кошелёк для получения EVER") : say("Seller", "Продавец")}: <code>{seller}</code></p>
    <p>{direction === "buy" ? say("USDT paying wallet", "Кошелёк оплаты USDT") : say("USDT recipient", "Получатель USDT")}: <code>{receivingAddress.trim()}</code></p>
    <p>{say("The 3% service fee is withheld from the USDT total and collected by the service; it is not a network fee. On-chain transfers stay at the gross amount.", "Комиссия сервиса 3% удерживается из суммы USDT и собирается сервисом; это не сетевой расход. Переводы в сети идут на полную сумму.")}</p>
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
  .reviewError { margin-top: 14px; padding: 10px 12px; border: 1px solid var(--color-danger); border-radius: 9px; color: var(--color-danger); font-size: 12px; line-height: 1.6; }
  .confirm { display: flex; justify-content: space-between; gap: 12px; width: 100%; margin-top: 18px; padding: 15px; border-radius: 9px; background: var(--color-accent); color: #152016; font-size: 12px; font-weight: 800; }
  .confirm:hover:not(:disabled) { background: var(--color-accent-strong); }
  .confirm:disabled { opacity: .45; cursor: default; }
  .back { display: block; width: 100%; margin-top: 9px; padding: 10px; color: var(--color-text-soft); font-size: 12px; }
</style>
