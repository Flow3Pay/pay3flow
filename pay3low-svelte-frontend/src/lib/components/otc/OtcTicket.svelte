<script lang="ts">
  import ExchangeFlowBridge from "../ExchangeFlowBridge.svelte";
  import { locale } from "$lib/i18n";
  import { otcCopy } from "$lib/otc/copy";
  import { rate, decimal, type Direction, type Trade } from "$lib/otc/service";
  export let direction: Direction = "buy";
  export let input = "100";
  export let quote: Trade | null = null;
  export let disabled = false, busy = false;
  export let desk = "";
  export let onDirection: (direction: Direction) => void;
  export let onEdit: () => void;
  export let onRequest: () => void;
  export let onReview: () => void;
  export let onAccount: () => void;
  export let accountLabel: string;
  $: copy = otcCopy($locale);
  $: say = (en: string, ru: string) => $locale === "ru" ? ru : en;
  $: sendAsset = direction === "buy" ? "USDT" : "EVER";
  $: receiveAsset = direction === "buy" ? "EVER" : "USDT";
</script>
{#snippet asset(asset: string)}
  <div class="methodControls assetIdentity"><img src={asset === "EVER" ? "/icons/assets/ever.svg" : "/icons/assets/usdt.png"} width="22" height="22" alt="" /><strong>{asset}</strong><span>{asset === "EVER" ? "Everscale" : "Ethereum"}</span></div>
{/snippet}
<form class="bridgeForm" on:submit|preventDefault={() => quote ? onReview() : onRequest()}>
  <div class="sideTabs" class:selling={direction === "sell"} role="group" aria-label="Trade direction"><button type="button" class:active={direction === "buy"} aria-pressed={direction === "buy"} on:click={() => onDirection("buy")}>{say("Buy EVER", "Купить EVER")}</button><button type="button" class:active={direction === "sell"} aria-pressed={direction === "sell"} on:click={() => onDirection("sell")}>{say("Sell EVER", "Продать EVER")}</button></div>
  <div class="quoteMode">{say("Fixed desk quote", "Фиксированная котировка деска")}<span>OTC</span></div>
  <label class="priceField"><span>{copy.price} · USDT / EVER</span><div><input aria-label={copy.price} readonly value={quote ? rate(quote.terms) : "—"} /><span>USDT</span></div></label>
  <p class="priceHint">{say("The desk quotes your exact input. Whole trades only.", "Деск котирует точную сумму. Только целые сделки.")}</p>
  <div class="intentLabel"><span>{copy.send}</span></div>
  <div class="moneyPanel exchangeMoneyPanel"><div class="panelCopy"><label for="otc-input">{say(`You send · ${sendAsset} on ${sendAsset === "EVER" ? "Everscale" : "Ethereum"}`, `Вы отправляете · ${sendAsset} · ${sendAsset === "EVER" ? "Everscale" : "Ethereum"}`)}</label><input id="otc-input" class="amountInput" type="text" inputmode="decimal" autocomplete="off" spellcheck="false" bind:value={input} on:input={onEdit} on:focus={(event) => event.currentTarget.select()} /></div>{@render asset(sendAsset)}</div>
  <ExchangeFlowBridge reversed={direction === "sell"} onSwap={() => onDirection(direction === "buy" ? "sell" : "buy")} label={`${copy.buy} / ${copy.sell}`} />
  <div class="intentLabel receiveLabel"><span>{copy.receive}</span></div>
  <div class="moneyPanel exchangeMoneyPanel"><div class="panelCopy"><label for="otc-receive">{say(`You receive · ${receiveAsset} on ${receiveAsset === "EVER" ? "Everscale" : "Ethereum"}`, `Вы получаете · ${receiveAsset} · ${receiveAsset === "EVER" ? "Everscale" : "Ethereum"}`)}</label><input id="otc-receive" class="amountInput amountOutput" readonly value={quote?.terms.output || ""} placeholder="—" /><small>{quote ? say("Fixed net output", "Фиксированная сумма к получению") : say("Awaiting fixed quote", "Ожидается котировка")}</small></div>{@render asset(receiveAsset)}</div>
  <div class="orderDetails"><div><span>{say("Desk", "Деск")}</span><strong>{desk || "—"}</strong></div><div><span>{copy.fees}</span><strong>{quote ? `${decimal(quote.terms.fee_policy.fee_units, 6)} USDT` : "max(0.25%, 5 USDT)"}</strong></div><small>{say("Completion fee paid by the desk. Network costs are separate.", "Комиссию за завершение платит деск. Сетевые расходы отдельно.")}</small></div>
  {#if quote}<p class="priceHint">{say("Quote expires", "Котировка истекает")}: {quote.terms.quote_by}</p>{/if}
  <button class="cta" type="submit" disabled={busy || disabled} data-testid="otc-review">{quote ? say("Review fixed quote", "Проверить котировку") : say("Request exact quote", "Запросить котировку")}<span aria-hidden="true">↗</span></button>
  <button class="accountButton" type="button" on:click={onAccount}>{accountLabel}</button>
</form>

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


.assetIdentity { display: flex; align-items: center; gap: 9px; padding: 12px; border: 1px solid var(--exchange-field-border); border-radius: 6px; background: var(--color-panel); font-size: 12px; } .assetIdentity span { margin-left: auto; color: var(--color-text-soft); } .quoteMode { display: flex; justify-content: space-between; padding: 17px 0 15px; font-size: 12px; } .quoteMode span { color: var(--color-text-soft); } .accountButton { display: block; width: 100%; min-height: 44px; margin-top: 8px; color: var(--color-text-soft); font-size: 12px; } .orderDetails > small { color: var(--color-text-soft); line-height: 1.5; }
</style>
