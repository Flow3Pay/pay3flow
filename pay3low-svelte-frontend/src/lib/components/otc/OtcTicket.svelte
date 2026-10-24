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
  $: copy = otcCopy($locale);
  $: say = (en: string, ru: string) => $locale === "ru" ? ru : en;
  $: sendAsset = direction === "buy" ? "USDT" : "EVER";
  $: receiveAsset = direction === "buy" ? "EVER" : "USDT";
  $: sendNetwork = direction === "buy" ? "Ethereum" : "Everscale";
  $: receiveNetwork = direction === "buy" ? "Everscale" : "Ethereum";
  $: sendLabel = `You send · ${sendAsset} on ${sendNetwork}`;
  $: receiveLabel = `You receive · ${receiveAsset} on ${receiveNetwork}`;
  $: fee = quote ? `${decimal(quote.terms.fee_policy.fee_units, 6)} USDT` : "max(0.25%, 5 USDT)";
</script>

{#snippet assetChip(asset: string, network: string)}
  <div class="assetChip"><img src={asset === "EVER" ? "/icons/assets/ever.svg" : "/icons/assets/usdt.png"} width="22" height="22" alt="" /><span><strong>{asset}</strong><small>{network}</small></span></div>
{/snippet}

<form class="bridgeForm" on:submit|preventDefault={() => quote ? onReview() : onRequest()}>
  <div class="sideTabs" class:selling={direction === "sell"} role="group" aria-label="Trade direction"><button type="button" class:active={direction === "buy"} aria-pressed={direction === "buy"} on:click={() => onDirection("buy")}>{say("Buy EVER", "Купить EVER")}</button><button type="button" class:active={direction === "sell"} aria-pressed={direction === "sell"} on:click={() => onDirection("sell")}>{say("Sell EVER", "Продать EVER")}</button></div>

  <div class="quoteMode"><span>{say("Fixed desk quote", "Фиксированная котировка деска")}</span><span class="otcTag">OTC</span></div>

  <div class="fieldHead"><span>{say("You pay", "Вы платите")}</span><span class="fieldHint">{sendAsset} · {sendNetwork}</span></div>
  <div class="moneyPanel exchangeMoneyPanel">
    <div class="panelCopy">
      <label for="otc-input">{sendLabel}</label>
      <input id="otc-input" class="amountInput" type="text" inputmode="decimal" autocomplete="off" spellcheck="false" bind:value={input} on:input={onEdit} on:focus={(event) => event.currentTarget.select()} />
      <small class="panelHint">{direction === "buy" ? say("USDT amount to convert", "Сумма USDT для обмена") : say("EVER amount to sell", "Сумма EVER к продаже")}</small>
    </div>
    {@render assetChip(sendAsset, sendNetwork)}
  </div>

  <ExchangeFlowBridge reversed={direction === "sell"} onSwap={() => onDirection(direction === "buy" ? "sell" : "buy")} label={`${copy.buy} / ${copy.sell}`} />

  <div class="fieldHead receiveHead"><span>{say("You receive", "Вы получаете")}</span><span class="fieldHint">{say("Fixed net output", "Фиксированная сумма")}</span></div>
  <div class="moneyPanel exchangeMoneyPanel receivePanel">
    <div class="panelCopy">
      <label for="otc-receive">{receiveLabel}</label>
      <input id="otc-receive" class="amountInput amountOutput" type="text" readonly value={quote?.terms.output || ""} placeholder="—" />
      <small class="panelHint">{quote ? say("Fixed net output", "Фиксированная сумма к получению") : say("Awaiting fixed quote", "Ожидается котировка")}</small>
    </div>
    {@render assetChip(receiveAsset, receiveNetwork)}
  </div>

  <div class="priceSummary">
    <div><span>{say("Price", "Цена")} <small>USDT / EVER</small></span><strong>{quote ? rate(quote.terms) : "—"}</strong></div>
    <div><span>{say("Desk fee", "Комиссия деска")}</span><strong>{fee}</strong></div>
    <div><span>{say("Desk", "Деск")}</span><strong>{desk || "—"}</strong></div>
  </div>
  <p class="feeNote">{say("Completion fee is paid by the desk. Network costs are separate.", "Комиссию за завершение платит деск. Сетевые расходы отдельно.")}</p>
  {#if quote}<p class="expiry"><span aria-hidden="true">◷</span>{say("Quote expires", "Котировка истекает")}: <strong>{quote.terms.quote_by}</strong></p>{/if}
  <button class="cta" type="submit" disabled={busy || disabled} data-testid="otc-review">{quote ? say("Review fixed quote", "Проверить котировку") : say("Request exact quote", "Запросить котировку")}<span aria-hidden="true">↗</span></button>
</form>

<style>
  .bridgeForm { display: flex; flex: 1; flex-direction: column; container-type: inline-size; container-name: exchange-bridge; padding: 0 17px 17px; }
  .sideTabs { position: relative; isolation: isolate; display: flex; gap: 4px; padding: 4px; border-radius: 9px; background: var(--color-panel); }
  .sideTabs::before { position: absolute; z-index: -1; inset: 4px auto 4px 4px; width: calc(50% - 6px); border-radius: 6px; background: var(--color-accent); content: ""; transition: transform .32s cubic-bezier(.22, 1, .36, 1); }
  .sideTabs.selling::before { transform: translateX(calc(100% + 4px)); }
  .sideTabs button { flex: 1; min-width: 0; padding: 9px 3px; border-radius: 6px; color: var(--color-text-soft); font-size: 12px; font-weight: 750; transition: color .2s ease; }
  .sideTabs button.active { color: #152016; }
  .quoteMode { display: flex; align-items: center; justify-content: space-between; gap: 8px; padding: 13px 0 11px; font-size: 11px; color: var(--color-text-soft); }
  .otcTag { padding: 2px 4px; border-radius: 3px; background: var(--color-panel); font-family: var(--font-mono); font-size: 10px; }
  .fieldHead { display: flex; align-items: baseline; justify-content: space-between; gap: 8px; margin: 0 2px 6px; }
  .fieldHead > span:first-child { font-size: 12px; color: var(--color-text); font-weight: 650; }
  .fieldHint { color: var(--color-text-faint); font-family: var(--font-mono); font-size: 9px; }
  .moneyPanel.exchangeMoneyPanel { min-height: 82px; padding: 8px 11px 7px; }
  .moneyPanel .amountInput { font-size: clamp(24px, 2.5vw, 30px); }
  .panelHint { font-size: 10px; color: var(--color-text-faint); }
  .assetChip { display: flex; align-items: center; gap: 8px; min-width: 0; padding: 6px 9px 6px 7px; border: 1px solid var(--exchange-asset-border); border-radius: 5px; background: var(--exchange-asset-bg); }
  .assetChip img { border-radius: 50%; flex: 0 0 auto; }
  .assetChip span { display: grid; gap: 1px; min-width: 0; }
  .assetChip strong { font-size: 12px; font-weight: 800; }
  .assetChip small { color: var(--color-text-soft); font-size: 9px; }
  .receiveHead { margin-top: 4px; }
  .receivePanel .amountInput { color: var(--color-text); }
  .priceSummary { display: grid; gap: 6px; padding: 11px 0 7px; }
  .priceSummary > div { display: flex; align-items: center; justify-content: space-between; gap: 10px; font-size: 11px; }
  .priceSummary span { color: var(--color-text-soft); }
  .priceSummary span small { font-family: var(--font-mono); font-size: 9px; }
  .priceSummary strong { max-width: 58%; overflow-wrap: anywhere; text-align: right; font-family: var(--font-mono); font-size: 11px; font-weight: 500; }
  .feeNote { font-size: 10px; color: var(--color-text-faint); line-height: 1.5; }
  .expiry { display: flex; align-items: center; gap: 6px; margin-top: 9px; padding: 7px 9px; border: 1px solid var(--color-border); border-radius: 5px; color: var(--color-text-soft); font-size: 10px; }
  .expiry strong { font-family: var(--font-mono); font-weight: 400; }
  .cta { display: flex; width: 100%; min-height: 48px; margin-top: auto; padding: 12px 15px; align-items: center; justify-content: space-between; border: 1px solid #b9e834; background: var(--color-accent); color: #171717; border-radius: 5px; font-size: 12px; font-weight: 800; transition: background .18s ease, transform .18s ease, opacity .18s ease; }
  .cta:hover:not(:disabled) { background: #c9f34b; transform: translateY(-1px); }
  .cta span { font-size: 17px; color: #2e2e2e; }
  :global(html[data-theme="dark"]) .cta { color: #152016; }
  :global(html[data-theme="dark"]) .cta span { color: #152016; }
  .cta:disabled { opacity: .4; cursor: default; }
  @media (max-width: 640px) { .bridgeForm { padding: 0 14px 15px; } .sideTabs button { padding: 11px; } .moneyPanel.exchangeMoneyPanel { min-height: 96px; } .moneyPanel .amountInput { font-size: 30px; } }
  @media (prefers-reduced-motion: reduce) { .sideTabs::before, .sideTabs button, .cta { transition: none; } }
</style>
