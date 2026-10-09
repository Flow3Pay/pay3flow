<script lang="ts">
  import { onMount } from "svelte";
  import { locale } from "$lib/i18n";
  import { lockPageScroll } from "$lib/page-scroll-lock";
  import { otcCopy } from "$lib/otc/copy";
  import { formatPrice, formatAmount, type OrderDraft, type OtcMarket } from "$lib/otc/model";
  export let draft: OrderDraft;
  export let market: OtcMarket;
  export let onClose: () => void;
  export let onConfirm: () => void;
  let dialog: HTMLDialogElement;
  $: copy = otcCopy($locale);
  onMount(() => {
    const previous = document.activeElement as HTMLElement | null;
    const unlock = lockPageScroll();
    dialog.showModal();
    return () => { dialog.close(); unlock(); previous?.focus(); };
  });
</script>
<dialog bind:this={dialog} class="reviewDialog" aria-labelledby="otc-review-title" on:cancel|preventDefault={onClose} on:click={(event) => { if (event.target === dialog) { const rect = dialog.getBoundingClientRect(); if (event.clientX < rect.left || event.clientX > rect.right || event.clientY < rect.top || event.clientY > rect.bottom) onClose(); } }}>
  <div class="reviewContent"><div class="dialogTop"><span>{copy.preview}</span><button type="button" aria-label={copy.close} on:click={onClose}><svg width="19" height="19" viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="m6 6 12 12M18 6 6 18" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" /></svg></button></div>
  <h2 id="otc-review-title">{copy.reviewTitle}</h2><p class="subtitle">{copy.reviewSubtitle}</p>
  <div class="orderSummary"><img src={market.icon} width="42" height="42" alt="" /><div><strong>{draft.side === "buy" ? copy.buy : copy.sell} {market.base}</strong><span>{draft.type === "limit" ? copy.limit : copy.marketOrder} · {market.base}/{market.quote}</span></div><span class="sideArrow" aria-hidden="true">↗</span></div>
  <dl><div><dt>{copy.price}</dt><dd>{formatPrice(draft.price, market)} {market.quote}</dd></div><div><dt>{copy.amount}</dt><dd>{formatAmount(draft.amount, market)} {market.base}</dd></div><div><dt>{copy.total}</dt><dd>{formatPrice(draft.total, market)} {market.quote}</dd></div><div><dt>{copy.fees}</dt><dd>{copy.feesValue}</dd></div></dl>
  <p class="demoNote">{copy.demoNote}</p>
  <button type="button" class="confirm" on:click={onConfirm}>{draft.type === "limit" ? copy.confirm : copy.simulate}<span aria-hidden="true">↗</span></button><button type="button" class="back" on:click={onClose}>{copy.back}</button>
  </div>
</dialog>
<style>
  .reviewDialog { position: fixed; width: min(430px, calc(100% - 32px)); max-height: calc(100dvh - 32px); padding: 0; margin: auto; overflow-y: auto; border: 1px solid var(--color-border); border-radius: 18px; background: var(--color-paper); color: var(--color-text); box-shadow: 0 24px 120px #0004; }
  .reviewDialog::backdrop { background: #0a100b99; backdrop-filter: blur(5px); }
  .reviewContent { padding: 24px; }
  .dialogTop { display: flex; justify-content: space-between; align-items: center; }
  .dialogTop > span { padding: 5px 8px; border: 1px solid var(--color-border); border-radius: 6px; font-family: var(--font-mono); font-size: 10px; color: var(--color-text-soft); }
  .dialogTop button { display: grid; width: 32px; height: 32px; place-items: center; border-radius: 7px; background: var(--color-panel); }
  h2 { margin-top: 20px; font-size: 25px; letter-spacing: -.05em; }
  .subtitle { margin: 7px 0 22px; font-size: 12px; color: var(--color-text-soft); }
  .orderSummary { display: flex; align-items: center; gap: 12px; padding: 16px; border-radius: 12px; background: var(--color-panel); }
  .orderSummary img { border-radius: 50%; }
  .orderSummary > div { display: grid; gap: 5px; }
  .orderSummary strong { font-size: 14px; }
  .orderSummary div > span { font-size: 10px; color: var(--color-text-soft); }
  .sideArrow { margin-left: auto; font-size: 28px; color: var(--color-accent-text); }
  dl { display: grid; gap: 14px; padding: 23px 0; }
  dl > div { display: flex; justify-content: space-between; gap: 12px; font-size: 11px; }
  dt { color: var(--color-text-soft); }
  dd { font-family: var(--font-mono); text-align: right; }
  .demoNote { border: 1px solid var(--color-border); border-radius: 9px; padding: 12px; color: var(--color-text-soft); font-size: 10px; line-height: 1.6; }
  .confirm { display: flex; justify-content: space-between; gap: 12px; width: 100%; margin-top: 18px; padding: 15px; border-radius: 9px; background: var(--color-accent); color: #152016; font-size: 12px; font-weight: 800; }
  .confirm:hover { background: var(--color-accent-strong); }
  .back { display: block; width: 100%; margin-top: 9px; padding: 10px; color: var(--color-text-soft); font-size: 12px; }
</style>
