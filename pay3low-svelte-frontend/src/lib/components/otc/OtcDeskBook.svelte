<script lang="ts">
  import { locale } from "$lib/i18n";
  import { proposalDirection } from "$lib/otc/terminal";
  import { bookLevels, bookSummary, compactNumber, percentChange, quoteObservations, trimDecimal } from "$lib/otc/market";
  import type { Config, Direction, Proposal, Trade } from "$lib/otc/service";
  export let config: Config | null;
  export let listings: Proposal[] = [];
  export let selectedId = "";
  export let trades: Trade[] = [];
  export let onSelect: (proposal: Proposal, direction: Direction) => void;
  $: say = (en: string, ru: string) => $locale === "ru" ? ru : en;
  $: offers = listings.map(proposal => ({ proposal, direction: proposalDirection(proposal, config) })).filter((item): item is { proposal: Proposal; direction: Direction } => item.direction !== null);
  $: observations = quoteObservations(trades);
  $: asks = bookLevels(observations, "buy");
  $: bids = bookLevels(observations, "sell");
  $: summary = bookSummary(observations);
  $: change = percentChange(observations);
  $: displayAsks = asks.slice(0, 7).reverse();
  $: displayBids = bids.slice(0, 7);
  $: maxDepth = Math.max(1, ...asks.slice(0, 7).map(level => level.depth), ...bids.slice(0, 7).map(level => level.depth));
  $: marketPair = "EVER / USDT";
  $: sourceLabel = observations.length ? say(`${observations.length} recorded ${observations.length === 1 ? "quote" : "quotes"}`, `${observations.length} записанных котировок`) : say("No quote levels yet", "Уровней пока нет");
  function priceLabel(value: number) {
    const decimals = value >= 100 ? 2 : value >= 1 ? 3 : value >= .01 ? 5 : 8;
    return value.toLocaleString("en-US", { minimumFractionDigits: decimals, maximumFractionDigits: decimals });
  }
  function amountLabel(value: string) { return trimDecimal(value, 4); }
  function totalLabel(value: string) { return trimDecimal(value, 2); }
</script>

<div class="bookTools">
  <div class="source"><span class="pulse" class:empty={!observations.length}></span>{sourceLabel}</div>
  <div class="bookMeta"><span>{say("Group", "Группа")} 0.0001</span><span aria-hidden="true">☰</span></div>
</div>
<div class="bookColumns"><span>{say("Price", "Цена")} <small>USDT</small></span><span>{say("Amount", "Объём")} <small>EVER</small></span><span>{say("Total", "Всего")} <small>USDT</small></span></div>

<div class="levels">
  <div class="askLevels" aria-label={say("Desk asks", "Предложения продажи")}>
    {#each displayAsks as level}
      <div class="level ask" style:--depth={`${Math.max(3, level.depth / maxDepth * 100)}%`}>
        <span class="levelPrice">{trimDecimal(level.price, 8)}</span><span>{amountLabel(level.amount)}</span><span>{totalLabel(level.total)}</span>
      </div>
    {/each}
    {#if !displayAsks.length}
      <div class="levelPlaceholder"><span aria-hidden="true"></span>{say("Asks appear after your first buy quote", "Продажи появятся после первой котировки покупки")}</div>
    {/if}
  </div>

  <div class="spread" class:nodata={!summary.mid}>
    <strong>{summary.mid ? trimDecimal(summary.mid, 8) : "—"}{#if change !== null}<span class:negative={change < 0}>{change >= 0 ? "↑" : "↓"} {Math.abs(change).toFixed(2)}%</span>{/if}</strong>
    <span><i class="buyMark"></i>{say("Bid", "Покупка")} {summary.bestBid === null ? "—" : priceLabel(summary.bestBid)}<i class="sellMark"></i>{say("Ask", "Продажа")} {summary.bestAsk === null ? "—" : priceLabel(summary.bestAsk)}</span>
  </div>

  <div class="bidLevels" aria-label={say("Desk bids", "Предложения покупки")}>
    {#each displayBids as level}
      <div class="level bid" style:--depth={`${Math.max(3, level.depth / maxDepth * 100)}%`}>
        <span class="levelPrice">{trimDecimal(level.price, 8)}</span><span>{amountLabel(level.amount)}</span><span>{totalLabel(level.total)}</span>
      </div>
    {/each}
    {#if !displayBids.length}
      <div class="levelPlaceholder"><span aria-hidden="true"></span>{say("Bids appear after your first sell quote", "Покупки появятся после первой котировки продажи")}</div>
    {/if}
  </div>
</div>

<div class="bookTally">
  <span>{say("Spread", "Спред")} <strong>{summary.spread === null ? "—" : priceLabel(summary.spread)}</strong>{#if summary.spreadPercent !== null}<small>({summary.spreadPercent.toFixed(3)}%)</small>{/if}</span>
  <span>{say("Depth", "Глубина")} <strong>{compactNumber(summary.bidDepth)} / {compactNumber(summary.askDepth)}</strong> <small>USDT</small></span>
</div>

<div class="offersHead"><span>{say("Desk offers", "Предложения деска")}</span><span>{marketPair}</span></div>
<div class="offers">
  {#each offers as { proposal, direction }}
    <button type="button" class="offerRow" class:sell={direction === "sell"} class:selected={proposal.id === selectedId} on:click={() => onSelect(proposal, direction)}>
      <span class="offerSide">{direction === "buy" ? say("Buy EVER", "Купить EVER") : say("Sell EVER", "Продать EVER")}</span>
      <span class="offerRoute">{direction === "buy" ? "USDT → EVER" : "EVER → USDT"}</span>
      <span class="offerAction">{say("Quote on request", "Котировка по запросу")}<i aria-hidden="true">↗</i></span>
    </button>
  {:else}
    <div class="empty"><span aria-hidden="true">≡</span><strong>{say("No desk offers", "Нет предложений деска")}</strong><p>{say("Published desk Proposals appear here when the route is configured.", "Опубликованные предложения появятся после настройки деска.")}</p></div>
  {/each}
</div>
<p class="bookFooter">{say("Private exact quotes · customer pays first", "Приватные точные котировки · клиент платит первым")}</p>

<style>
  .bookTools { display: flex; align-items: center; justify-content: space-between; gap: 10px; padding: 9px 15px; border-bottom: 1px solid var(--color-border); }
  .source { display: flex; align-items: center; gap: 7px; font-size: 11px; color: var(--color-text-soft); }
  .pulse { width: 6px; height: 6px; border-radius: 50%; background: var(--otc-buy); box-shadow: 0 0 0 3px var(--otc-buy-soft); }
  .pulse.empty { background: var(--color-text-faint); box-shadow: none; }
  .bookMeta { display: flex; align-items: center; gap: 9px; color: var(--color-text-soft); font-family: var(--font-mono); font-size: 10px; }
  .bookColumns, .level { display: grid; grid-template-columns: 1.05fr .9fr 1fr; align-items: center; text-align: right; gap: 7px; }
  .bookColumns { padding: 9px 15px 7px; font-size: 11px; color: var(--color-text-soft); }
  .bookColumns > span:first-child, .level > span:first-child { text-align: left; }
  .bookColumns small { display: block; margin-top: 2px; font-size: 10px; opacity: .7; }
  .levels { min-height: 232px; }
  .level { position: relative; isolation: isolate; width: 100%; height: 26px; padding: 0 15px; font-family: var(--font-mono); font-size: 11px; font-variant-numeric: tabular-nums; }
  .level::before { content: ""; position: absolute; z-index: -1; inset: 1px 0 1px auto; width: var(--depth); border-radius: 2px 0 0 2px; background: var(--otc-buy-soft); }
  .level.bid { color: var(--otc-buy); }
  .level.ask { color: var(--otc-sell); }
  .level.ask::before { background: var(--otc-sell-soft); }
  .level:hover { background: var(--color-panel); }
  .levelPlaceholder { display: flex; align-items: center; justify-content: center; min-height: 46px; padding: 8px 15px; color: var(--color-text-faint); font-size: 10px; line-height: 1.5; text-align: center; border-bottom: 1px dashed var(--color-border); }
  .spread { display: flex; align-items: center; justify-content: space-between; flex-wrap: wrap; gap: 4px 10px; min-height: 52px; margin: 3px 0; padding: 9px 15px; border-block: 1px solid var(--color-border); }
  .spread strong { display: flex; align-items: baseline; gap: 7px; color: var(--otc-buy); font-family: var(--font-mono); font-size: 19px; font-weight: 500; letter-spacing: -.04em; }
  .spread strong span { font-size: 11px; letter-spacing: 0; }
  .spread strong span.negative { color: var(--otc-sell); }
  .spread > span { display: flex; align-items: center; gap: 5px; color: var(--color-text-soft); font-family: var(--font-mono); font-size: 9px; }
  .spread.nodata strong { color: var(--color-text-soft); }
  .buyMark, .sellMark { width: 5px; height: 5px; border-radius: 50%; background: var(--otc-buy); }
  .sellMark { background: var(--otc-sell); margin-left: 4px; }
  .bookTally { display: flex; justify-content: space-between; gap: 10px; padding: 8px 15px; border-bottom: 1px solid var(--color-border); color: var(--color-text-soft); font-size: 10px; }
  .bookTally strong { font-family: var(--font-mono); font-weight: 400; color: var(--color-text); }
  .bookTally small { font-size: 10px; }
  .offersHead { display: flex; justify-content: space-between; gap: 8px; padding: 12px 15px 7px; color: var(--color-text-soft); font-size: 10px; font-family: var(--font-mono); text-transform: uppercase; letter-spacing: .06em; }
  .offers { display: grid; gap: 6px; padding: 0 11px 10px; }
  .offerRow { display: grid; grid-template-columns: 1fr auto; gap: 2px 8px; width: 100%; min-height: 46px; padding: 7px 9px; border: 1px solid var(--color-border); border-radius: 6px; background: var(--color-paper); text-align: left; }
  .offerRow:hover, .offerRow.selected { border-color: var(--color-accent-strong); background: var(--otc-buy-soft); }
  .offerRow.sell:hover, .offerRow.sell.selected { background: var(--otc-sell-soft); }
  .offerSide { color: var(--otc-buy); font-size: 12px; font-weight: 750; }
  .offerRow.sell .offerSide { color: var(--otc-sell); }
  .offerRoute { grid-column: 1; color: var(--color-text-soft); font-family: var(--font-mono); font-size: 10px; }
  .offerAction { display: flex; align-items: center; gap: 5px; grid-column: 2; grid-row: 1 / span 2; color: var(--color-text-soft); font-size: 10px; }
  .offerAction i { font-style: normal; font-size: 13px; }
  .empty { display: grid; gap: 8px; padding: 26px 12px; text-align: center; justify-items: center; font-size: 12px; }
  .empty > span { font-size: 24px; color: var(--color-text-faint); }
  .empty p { max-width: 230px; color: var(--color-text-soft); line-height: 1.5; font-size: 11px; }
  .bookFooter { padding: 9px 15px 12px; border-top: 1px solid var(--color-border); color: var(--color-text-soft); font-size: 10px; line-height: 1.5; }
  @media (max-width: 1100px) { .level { font-size: 10px; } }
  @media (pointer: coarse) { .offerRow { min-height: 60px; } }
</style>
