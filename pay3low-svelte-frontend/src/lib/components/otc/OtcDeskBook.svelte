<script lang="ts">
  import { onMount, tick } from "svelte";
  import { locale } from "$lib/i18n";
  import { proposalDirection } from "$lib/otc/terminal";
  import type { Config, Direction, Proposal } from "$lib/otc/service";
  import { saleRate, compareOfferRates, offerDirection, offerOwner, type PublicOffer } from "$lib/otc/public-offers";
  export let publicOffers: PublicOffer[] = [], ownIds: string[] = [];
  export let onCancel: (id: string) => void = () => {};
  export let busy = false, compact = false, showEmpty = true;
  export let dialogOnly = false;
  export let testId = "otc-public-offer";
  export let config: Config | null;
  export let listings: Proposal[] = [];
  export let selectedId = "";
  export let onSelect: (proposal: Proposal, direction: Direction) => void;
  let selectedOffer: PublicOffer | null = null;
  let dialog: HTMLDialogElement;
  let copied = "";
  $: say = (en: string, ru: string) => $locale === "ru" ? ru : en;
  $: offers = listings.map(proposal => ({ proposal, direction: proposalDirection(proposal, config) })).filter(item => item.direction);
  $: sortedOffers = compact ? publicOffers : [...publicOffers].sort((a,b) => {
    const side = offerDirection(a);
    if (side !== offerDirection(b)) return side === "sell" ? -1 : 1;
    return compareOfferRates(a.terms.ever_amount,a.terms.usdt_amount,b.terms.ever_amount,b.terms.usdt_amount) * (side === "sell" ? 1 : -1);
  });
  $: maxBuy = Math.max(1, ...sortedOffers.filter(o => offerDirection(o) === "buy").map(o => Number(o.terms.usdt_amount) || 0));
  $: maxSell = Math.max(1, ...sortedOffers.filter(o => offerDirection(o) === "sell").map(o => Number(o.terms.usdt_amount) || 0));

  async function showOffer(offer: PublicOffer) {
    selectedOffer = offer; copied = "";
    await tick();
    if (dialog && !dialog.open) dialog.showModal();
  }
  export function openOffer(offer: PublicOffer) { void showOffer(offer); }
  function closeOffer() {
    if (dialog?.open) dialog.close();
    selectedOffer = null; copied = "";
  }
  async function copyAddress(address: string, key: string) {
    try { await navigator.clipboard.writeText(address); copied = key; }
    catch { copied = ""; }
  }
  onMount(() => () => { if (dialog?.open) dialog.close(); });
</script>

{#if !dialogOnly}
{#if !compact}<div class="bookHeader" aria-hidden="true"><span>{say("Price", "Цена")}<small>USDT / EVER</small></span><span>{say("Amount", "Объём")}<small>EVER</small></span><span>{say("Total", "Всего")}<small>USDT</small></span></div>{/if}
<div class="bookRows" class:compact>
  {#each sortedOffers as offer, index (offer.id)}
    {@const side = offerDirection(offer)}
    {@const isOwn = ownIds.includes(offer.id)}
    {@const max = side === "buy" ? maxBuy : maxSell}
    {#if !compact && (index === 0 || offerDirection(sortedOffers[index-1]) !== side)}<h3 class="bookSide" class:sell={side === "sell"}>{side === "buy" ? say("Buy offers", "Покупают") : say("Sell offers", "Продают")}</h3>{/if}
    <button type="button" class="bookRow" class:sell={side === "sell"} data-testid={testId} aria-label={`${say("Offer details", "Детали оффера")}: ${saleRate(offer.terms.ever_amount,offer.terms.usdt_amount)} USDT / EVER`} on:click={() => showOffer(offer)}>
      <span class="depth" style={`--depth:${Math.max(4, Math.min(100, (Number(offer.terms.usdt_amount) || 0) / max * 100))}%`}></span>
      <span class="rowPrice" data-testid="otc-book-price">{saleRate(offer.terms.ever_amount,offer.terms.usdt_amount)}</span>
      <span class="rowAmount">{offer.terms.ever_amount}<small>EVER</small></span>
      <span class="rowTotal">{offer.terms.usdt_amount}<small>USDT</small></span>
      <span class="rowOwner"><span class="ownerDot" aria-hidden="true"></span><code title={offerOwner(offer)}>{offerOwner(offer).slice(0,6)}…{offerOwner(offer).slice(-4)}</code>{#if isOwn}<span class="ownTag">{say("You", "Вы")}</span>{/if}</span>
      <svg class="rowChevron" width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="m6 3 5 5-5 5" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>
    </button>
  {/each}
  {#each offers as { proposal, direction }}
    <div class="proposalRow" class:sell={direction === "sell"} class:selected={proposal.id === selectedId}>
      <button type="button" class="proposalSelect" on:click={() => onSelect(proposal, direction!)}><span class="proposalSide">{direction === "buy" ? say("Buy EVER", "Купить EVER") : say("Sell EVER", "Продать EVER")}</span><span>{say("Desk quote", "Котировка деска")}</span><svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="m6 3 5 5-5 5" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg></button>
      <p>{direction === "buy" ? "USDT · Ethereum → EVER · Everscale" : "EVER · Everscale → USDT · Ethereum"}</p>
      <a href={proposal.id} target="_blank" rel="noreferrer">{proposal.name || say("Desk proposal", "Предложение деска")}</a>
    </div>
  {:else}
    {#if !publicOffers.length && showEmpty}<div class="empty"><span class="emptyIcon" aria-hidden="true"><svg width="20" height="20" viewBox="0 0 20 20" fill="none"><path d="M3 5.5h14M3 10h14M3 14.5h9" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg></span><strong>{config?.public_offers_available ? say("No open offers", "Пока нет офферов") : say("No desk offers", "Нет предложений деска")}</strong>{#if config?.public_offers_available}<p>{say("Be the first to post an offer.", "Опубликуйте первый оффер.")}</p>{/if}</div>{/if}
  {/each}
</div>
{#if !compact}<p class="bookFooter">{config?.public_offers_available && !config.available ? say("Offers are public · settlement on this site is paused", "Офферы публичны · расчёты через сайт отключены") : say("Public offers · USDT / EVER", "Публичные офферы · USDT / EVER")}</p>{/if}
{/if}

{#if selectedOffer}
  {@const selected = selectedOffer}
  {@const side = offerDirection(selected)}
  {@const isOwn = ownIds.includes(selected.id)}
  <dialog bind:this={dialog} class="offerDialog" aria-labelledby="otc-offer-details-title" on:cancel|preventDefault={closeOffer} on:click={(event) => { if (event.target === dialog) closeOffer(); }}>
    <div class="dialogHead"><div><span class="dialogEyebrow" class:sell={side === "sell"}>{side === "buy" ? say("BUY OFFER", "ОФФЕР НА ПОКУПКУ") : say("SELL OFFER", "ОФФЕР НА ПРОДАЖУ")}</span><h2 id="otc-offer-details-title">{say("Offer details", "Детали оффера")}</h2></div><button type="button" class="iconButton" aria-label={say("Close", "Закрыть")} on:click={closeOffer}><svg width="18" height="18" viewBox="0 0 20 20" fill="none" aria-hidden="true"><path d="m5 5 10 10M15 5 5 15" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/></svg></button></div>
    <div class="dialogSummary"><div><span>{say("Price", "Цена")}</span><strong>{saleRate(selected.terms.ever_amount,selected.terms.usdt_amount)} <small>USDT / EVER</small></strong></div><div><span>{say("Amount", "Количество")}</span><strong>{selected.terms.ever_amount} <small>EVER</small></strong></div><div><span>{say("Total", "Сумма")}</span><strong>{selected.terms.usdt_amount} <small>USDT</small></strong></div></div>
    <div class="addressList">
      <div class="addressRow"><img src="/icons/assets/ever.svg" alt="" width="22" height="22"><div><span>{side === "buy" ? say("EVER receiving wallet", "Кошелёк получения EVER") : say("Seller EVER wallet", "EVER-кошелёк продавца")}</span><code>{offerOwner(selected)}</code></div><button class="copyButton" type="button" aria-label={say("Copy EVER address", "Копировать EVER-адрес")} on:click={() => copyAddress(offerOwner(selected), "ever")}>{#if copied === "ever"}<span>{say("Copied", "Скопировано")}</span>{:else}<svg width="16" height="16" viewBox="0 0 20 20" fill="none" aria-hidden="true"><rect x="7" y="7" width="9" height="10" rx="1.5" stroke="currentColor" stroke-width="1.5"/><path d="M12 7V4.5A1.5 1.5 0 0 0 10.5 3h-6A1.5 1.5 0 0 0 3 4.5v8A1.5 1.5 0 0 0 4.5 14H7" stroke="currentColor" stroke-width="1.5"/></svg>{/if}</button></div>
      <div class="addressRow"><img src="/icons/assets/usdt.png" alt="" width="22" height="22"><div><span>{side === "buy" ? say("USDT paying wallet", "Кошелёк оплаты USDT") : say("USDT receiving wallet", "Кошелёк получения USDT")}</span><code>{selected.terms.usdt_address}</code></div><button class="copyButton" type="button" aria-label={say("Copy Ethereum address", "Копировать Ethereum-адрес")} on:click={() => copyAddress(selected.terms.usdt_address, "usdt")}>{#if copied === "usdt"}<span>{say("Copied", "Скопировано")}</span>{:else}<svg width="16" height="16" viewBox="0 0 20 20" fill="none" aria-hidden="true"><rect x="7" y="7" width="9" height="10" rx="1.5" stroke="currentColor" stroke-width="1.5"/><path d="M12 7V4.5A1.5 1.5 0 0 0 10.5 3h-6A1.5 1.5 0 0 0 3 4.5v8A1.5 1.5 0 0 0 4.5 14H7" stroke="currentColor" stroke-width="1.5"/></svg>{/if}</button></div>
    </div>
    <div class="dialogFoot"><span><svg width="15" height="15" viewBox="0 0 20 20" fill="none" aria-hidden="true"><circle cx="10" cy="10" r="7" stroke="currentColor" stroke-width="1.4"/><path d="M10 6v4l2.5 1.5" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>{say("Open until", "Действует до")} {new Date(selected.expires_at).toLocaleString($locale)}</span>{#if isOwn && selected.state === "open"}<button type="button" class="cancelOffer" disabled={busy} on:click={() => { onCancel(selected.id); closeOffer(); }}>{say("Cancel offer", "Снять оффер")}</button>{/if}</div>
    {#if config?.public_offers_available && !config.available}<p class="settlementNote"><svg width="16" height="16" viewBox="0 0 20 20" fill="none" aria-hidden="true"><circle cx="10" cy="10" r="7" stroke="currentColor" stroke-width="1.4"/><path d="M10 9v4m0-6h.01" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg>{say("Settlement through this site is currently unavailable. Verify addresses with the other party before sending funds.", "Расчёты через сайт сейчас недоступны. Перед переводом сверьте адреса с контрагентом.")}</p>{/if}
  </dialog>
{/if}

<style>
  .bookRows.compact { min-height:0; flex:0 0 auto; }
  .bookSide { margin:0; padding:9px var(--otc-panel-padding,20px); border-bottom:1px solid var(--color-border); background:var(--otc-buy-soft); color:var(--otc-buy); font-size:11px; font-weight:700; letter-spacing:.045em; text-transform:uppercase; }
  .bookSide.sell { color:var(--otc-sell); background:var(--otc-sell-soft); }
  .bookHeader, .bookRow { display:grid; grid-template-columns:minmax(0,1fr) minmax(0,.8fr) minmax(0,.8fr) 14px; gap:8px; align-items:center; }
  .bookHeader { min-height:44px; padding:8px var(--otc-panel-padding,20px); border-bottom:1px solid var(--color-border); color:var(--color-text-soft); font-size:11px; line-height:1.4; text-align:right; }
  .bookHeader > span:first-child { text-align:left; }
  .bookHeader small { display:block; color:var(--color-text-faint); font-size:10px; }
  .bookRows { display:flex; flex:1; flex-direction:column; min-height:240px; }
  .bookRow { position:relative; isolation:isolate; width:100%; min-height:52px; padding:8px var(--otc-panel-padding,20px); border-bottom:1px solid var(--color-border); background:transparent; color:var(--color-text); font-family:var(--font-mono); font-size:12px; font-variant-numeric:tabular-nums; text-align:right; cursor:pointer; }
  .bookRow:hover { background:var(--color-panel); }
  .depth { position:absolute; z-index:-1; inset:3px 0 3px auto; width:var(--depth); border-radius:5px 0 0 5px; background:var(--otc-buy-soft); pointer-events:none; }
  .bookRow.sell .depth { background:var(--otc-sell-soft); }
  .rowPrice { justify-self:start; color:var(--otc-buy); font-weight:650; }
  .bookRow.sell .rowPrice { color:var(--otc-sell); }
  .rowAmount,.rowTotal { overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .rowAmount small,.rowTotal small { margin-left:4px; color:var(--color-text-faint); font-family:var(--font-sans,inherit); font-size:9px; }
  .rowOwner { display:none; grid-column:1/4; align-items:center; gap:6px; color:var(--color-text-soft); text-align:left; }
  .rowOwner code { overflow:hidden; text-overflow:ellipsis; white-space:nowrap; font-size:10px; }
  .ownerDot { width:5px; height:5px; flex:0 0 auto; border-radius:50%; background:var(--color-text-faint); }
  .ownTag { padding:2px 5px; border-radius:4px; background:var(--color-panel); color:var(--color-text-soft); font-family:var(--font-sans,inherit); font-size:9px; }
  .rowChevron { color:var(--color-text-faint); }
  .bookRow:focus-visible,.proposalSelect:focus-visible,.copyButton:focus-visible,.iconButton:focus-visible,.cancelOffer:focus-visible { outline:var(--focus-ring-width) solid var(--color-focus); outline-offset:2px; }
  .proposalRow { padding:10px var(--otc-panel-padding,20px); border-bottom:1px solid var(--color-border); }
  .proposalRow.selected { background:var(--otc-buy-soft); }
  .proposalSelect { display:flex; width:100%; min-width:0; min-height:38px; align-items:center; gap:8px; text-align:left; font-size:12px; }
  .proposalSelect > span:nth-child(2) { margin-left:auto; color:var(--color-text-soft); font-size:11px; }
  .proposalSide { color:var(--otc-buy); font-weight:650; }
  .proposalRow.sell .proposalSide { color:var(--otc-sell); }
  .proposalRow p { margin:4px 0; color:var(--color-text-soft); font-size:11px; }
  .proposalRow a { color:var(--color-text-soft); font-size:11px; text-decoration:underline; text-underline-offset:3px; }
  .empty { display:grid; flex:1; min-height:240px; align-content:center; justify-items:center; gap:7px; padding:24px var(--otc-panel-padding,20px); text-align:center; }
  .emptyIcon { display:grid; width:38px; height:38px; place-items:center; margin-bottom:2px; border:1px solid var(--color-border); border-radius:10px; background:var(--color-panel); color:var(--color-text-soft); }
  .empty strong { font-size:13px; font-weight:650; }
  .empty p { margin:0; color:var(--color-text-soft); font-size:12px; }
  .bookFooter { display:flex; flex:0 0 auto; min-height:42px; align-items:center; padding:9px var(--otc-panel-padding,20px); border-top:1px solid var(--color-border); color:var(--color-text-soft); font-size:10px; line-height:1.45; }
  .offerDialog { width:min(460px,calc(100% - 32px)); max-height:min(86dvh,720px); padding:0; overflow:auto; border:1px solid var(--color-border-strong); border-radius:14px; background:var(--color-paper); color:var(--color-text); box-shadow:0 24px 80px #0005; }
  .offerDialog::backdrop { background:#10151bcc; backdrop-filter:blur(4px); }
  .dialogHead { display:flex; align-items:flex-start; justify-content:space-between; gap:16px; padding:20px 22px 16px; border-bottom:1px solid var(--color-border); }
  .dialogEyebrow { color:var(--otc-buy); font-family:var(--font-mono); font-size:10px; font-weight:700; letter-spacing:.08em; }
  .dialogEyebrow.sell { color:var(--otc-sell); }
  .dialogHead h2 { margin:5px 0 0; font-size:19px; letter-spacing:-.03em; }
  .iconButton { display:grid; width:36px; height:36px; flex:0 0 auto; place-items:center; border:1px solid var(--color-border); border-radius:9px; color:var(--color-text-soft); }
  .dialogSummary { display:grid; grid-template-columns:repeat(3,minmax(0,1fr)); gap:12px; padding:18px 22px; border-bottom:1px solid var(--color-border); }
  .dialogSummary > div { display:grid; min-width:0; gap:6px; }
  .dialogSummary span,.addressRow > div > span { color:var(--color-text-soft); font-size:11px; }
  .dialogSummary strong { overflow-wrap:anywhere; font-family:var(--font-mono); font-size:14px; font-weight:600; }
  .dialogSummary small { display:block; margin-top:3px; color:var(--color-text-faint); font-family:var(--font-sans,inherit); font-size:10px; font-weight:400; }
  .addressList { display:grid; padding:4px 22px; }
  .addressRow { display:grid; grid-template-columns:22px minmax(0,1fr) 36px; align-items:center; gap:10px; min-width:0; padding:13px 0; border-bottom:1px solid var(--color-border); }
  .addressRow > div { display:grid; min-width:0; gap:5px; }
  .addressRow code { overflow-wrap:anywhere; color:var(--color-text); font-family:var(--font-mono); font-size:11px; line-height:1.5; }
  .copyButton { display:grid; min-width:36px; min-height:36px; place-items:center; border:1px solid var(--color-border); border-radius:8px; color:var(--color-text-soft); font-size:10px; }
  .dialogFoot { display:flex; align-items:center; justify-content:space-between; gap:12px; padding:14px 22px; color:var(--color-text-soft); font-size:11px; }
  .dialogFoot > span { display:flex; align-items:center; gap:6px; }
  .cancelOffer { flex:0 0 auto; min-height:36px; padding:7px 10px; border:1px solid var(--color-border); border-radius:8px; color:var(--color-text); font-size:11px; }
  .cancelOffer:disabled { opacity:.5; cursor:default; }
  .settlementNote { display:flex; align-items:flex-start; gap:8px; margin:0; padding:12px 22px 18px; color:var(--color-text-soft); font-size:11px; line-height:1.55; }
  .settlementNote svg { flex:0 0 auto; color:var(--color-accent-strong); }
  @media (min-width:640px) { .rowOwner { display:flex; } }
  @media (max-width:520px) { .bookHeader,.bookRow { grid-template-columns:minmax(0,1fr) minmax(0,.8fr) minmax(0,.75fr) 12px; gap:5px; } .bookHeader,.bookRow { padding-right:12px; padding-left:12px; } .rowAmount small,.rowTotal small { display:none; } .dialogHead,.dialogSummary,.addressList { padding-right:16px; padding-left:16px; } .settlementNote { padding-right:16px; padding-left:16px; } .dialogFoot { align-items:flex-start; padding-right:16px; padding-left:16px; } }
  @media (prefers-reduced-motion:reduce) { .bookRow,.proposalSelect,.copyButton { transition:none; } }
</style>
