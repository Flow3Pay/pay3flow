<script lang="ts">
  import { tick } from "svelte";
  import { locale } from "$lib/i18n";
  import { otcCopy } from "$lib/otc/copy";
  import { formatPrice, formatAmount, type OtcMarket, type OtcSnapshot, type BookLevel, type OrderSide } from "$lib/otc/model";
  export let market: OtcMarket;
  export let snapshot: OtcSnapshot;
  export let onSelect: (side: OrderSide, price: number) => void;
  export let selectedPrice: number | null = null;
  export let sideOnly: OrderSide | null = null;
  export let showTools = true;
  $: effectiveFilter = sideOnly === "buy" ? "bids" : sideOnly === "sell" ? "asks" : filter;
  let filter: "all" | "bids" | "asks" = "all";
  let grouping = 1;
  let groupingOpen = false;
  let groupingWrap: HTMLDivElement;
  let groupingButton: HTMLButtonElement;
  function closeGrouping() { groupingOpen = false; groupingButton?.focus(); }
  async function toggleGrouping() {
    groupingOpen = !groupingOpen;
    if (groupingOpen) { await tick(); groupingWrap.querySelector<HTMLButtonElement>('[aria-pressed="true"]')?.focus(); }
  }
  function outsideClick(event: MouseEvent) { if (groupingOpen && !groupingWrap?.contains(event.target as Node)) groupingOpen = false; }
  function groupingKeys(event: KeyboardEvent) {
    if (!groupingOpen) return;
    if (event.key === "Escape") { event.preventDefault(); event.stopImmediatePropagation(); closeGrouping(); }
    else if (["ArrowUp", "ArrowDown", "Home", "End"].includes(event.key)) {
      event.preventDefault();
      const options = [...groupingWrap.querySelectorAll<HTMLButtonElement>('.groupingMenu button')];
      const index = options.indexOf(document.activeElement as HTMLButtonElement);
      const next = event.key === "Home" ? 0 : event.key === "End" ? options.length - 1 : (index + (event.key === "ArrowDown" ? 1 : -1) + options.length) % options.length;
      options[next]?.focus();
    }
  }
  $: copy = otcCopy($locale);
  $: asks = group(snapshot.asks, "sell", grouping);
  $: bids = group(snapshot.bids, "buy", grouping);
  $: maxDepth = Math.max(...asks.map((level) => level.depth), ...bids.map((level) => level.depth));
  $: spread = snapshot.asks[0].price - snapshot.bids[0].price;
  $: bidDepth = snapshot.bids.at(-1)?.depth ?? 0;
  $: askDepth = snapshot.asks.at(-1)?.depth ?? 0;
  $: buyRatio = Math.round(bidDepth / (bidDepth + askDepth) * 100);
  function group(levels: BookLevel[], side: OrderSide, multiplier: number) {
    if (multiplier === 1) return levels;
    const bucketSize = market.tickSize * multiplier;
    const buckets = new Map<number, BookLevel>();
    for (const level of levels) {
      const price = Number(((side === "sell" ? Math.ceil(level.price / bucketSize) : Math.floor(level.price / bucketSize)) * bucketSize).toFixed(market.priceDecimals));
      const existing = buckets.get(price);
      buckets.set(price, { price, amount: (existing?.amount ?? 0) + level.amount, total: (existing?.total ?? 0) + level.total, depth: 0 });
    }
    let depth = 0;
    return [...buckets.values()].map((level) => ({ ...level, depth: depth += level.total }));
  }
</script>
<svelte:window on:mousedown={outsideClick} on:keydown={groupingKeys} />
<div class="orderbook" class:split={sideOnly !== null}>
{#if showTools}<div class="bookTools"><div class="bookFilters" role="group" aria-label={copy.book}>
  {#each ["all", "bids", "asks"] as item}<button type="button" class:active={filter === item} aria-label={item === "all" ? copy.all : item === "bids" ? copy.bids : copy.asks} aria-pressed={filter === item} on:click={() => filter = item as typeof filter}><svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true"><path d="M2 3h4M2 6h4M2 9h4M2 12h4" stroke={item === "asks" ? "var(--otc-sell)" : "var(--otc-buy)"} stroke-width="2.3" /><path d="M9 3h5M9 6h5M9 9h5M9 12h5" stroke={item === "bids" ? "var(--otc-buy)" : "var(--otc-sell)"} stroke-width="2.3" /></svg></button>{/each}
</div><div class="groupingWrap" bind:this={groupingWrap}><button type="button" class="groupingButton" bind:this={groupingButton} aria-label={copy.grouping} aria-haspopup="dialog" aria-expanded={groupingOpen} on:click={toggleGrouping}>{formatPrice(market.tickSize * grouping, market)}<svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="m4 6 4 4 4-4" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" /></svg></button>{#if groupingOpen}<div class="groupingMenu" role="dialog" aria-label={copy.grouping} tabindex="-1">{#each [1, 2, 5] as value}<button type="button" class:active={grouping === value} aria-pressed={grouping === value} on:click={() => { grouping = value; closeGrouping(); }}><span>{formatPrice(market.tickSize * value, market)}</span>{#if grouping === value}<span aria-hidden="true">✓</span>{/if}</button>{/each}</div>{/if}</div></div>{/if}
<div class="bookColumns"><span>{copy.price} <small>{market.quote}</small></span><span>{copy.amount} <small>{market.base}</small></span><span>{copy.total} <small>{market.quote}</small></span></div>
<div class="levels" class:oneSide={effectiveFilter !== "all"}>
  {#if effectiveFilter !== "bids"}<div class="askLevels" aria-label={copy.asks}>{#each (sideOnly ? asks : [...(effectiveFilter === "all" ? asks.slice(0, 7) : asks)].reverse()) as level}<button type="button" class="level ask" class:selected={selectedPrice === level.price} style:--depth={`${level.depth / maxDepth * 100}%`} aria-label={`${copy.buy} ${market.base} · ${formatPrice(level.price, market)} ${market.quote}`} on:click={() => onSelect("buy", level.price)}><span class="levelPrice">{formatPrice(level.price, market)}</span><span>{formatAmount(level.amount, market)}</span><span>{level.total.toLocaleString("en-US", { maximumFractionDigits: 0 })}</span></button>{/each}</div>{/if}
  {#if !sideOnly}<div class="spread"><strong>{formatPrice(market.price, market)}<span aria-hidden="true">↗</span></strong><span>{copy.spread} {formatPrice(spread, market)} <small>({(spread / market.price * 100).toFixed(3)}%)</small></span></div>{/if}
  {#if effectiveFilter !== "asks"}<div class="bidLevels" aria-label={copy.bids}>{#each (effectiveFilter === "all" ? bids.slice(0, 7) : bids) as level}<button type="button" class="level bid" class:selected={selectedPrice === level.price} style:--depth={`${level.depth / maxDepth * 100}%`} aria-label={`${copy.sell} ${market.base} · ${formatPrice(level.price, market)} ${market.quote}`} on:click={() => onSelect("sell", level.price)}><span class="levelPrice">{formatPrice(level.price, market)}</span><span>{formatAmount(level.amount, market)}</span><span>{level.total.toLocaleString("en-US", { maximumFractionDigits: 0 })}</span></button>{/each}</div>{/if}
</div>
{#if !sideOnly}<div class="pressure"><div class="pressureLabels"><span>{copy.buyPressure} {buyRatio}%</span><span>{100 - buyRatio}% {copy.sellPressure}</span></div><div class="pressureBar"><span style:width={`${buyRatio}%`}></span></div></div>{/if}
</div>
<style>
  .orderbook { container-type: inline-size; container-name: orderbook; font-family: var(--font-sans); font-style: normal; }
  @container orderbook (max-width: 260px) {
    .split .bookColumns, .split .level { grid-template-columns: minmax(0, 1.1fr) minmax(0, 1fr); gap: 4px; padding-inline: 6px; font-size: 11px; }
    .split .bookColumns > span:last-child, .split .level > span:last-child { display: none; }
    .split .bookColumns small { font-size: 10px; }
  }

  .bookTools { display: flex; justify-content: space-between; align-items: center; padding: 12px 14px 9px; }
  .bookFilters { display: flex; gap: 3px; }
  .bookFilters button { display: grid; place-items: center; width: 26px; height: 25px; border-radius: 5px; opacity: .45; }
  .bookFilters button.active { opacity: 1; background: var(--color-panel); }
  .groupingWrap { position: relative; }
  .groupingButton { display: flex; align-items: center; gap: 8px; min-height: 32px; padding: 4px 7px; border: 1px solid var(--color-border); border-radius: 5px; background: var(--color-paper); font-family: var(--font-sans); font-size: 12px; }
  .groupingMenu { position: absolute; z-index: 80; top: calc(100% + 9px); right: 0; width: 160px; padding: 10px; border: 1px solid var(--color-border); border-radius: 20px; background: var(--color-paper); animation: groupingIn .18s ease-out; }
  .groupingMenu button { display: flex; align-items: center; justify-content: space-between; width: 100%; min-height: 40px; padding: 8px 10px; border-radius: 7px; font-family: var(--font-sans); font-size: 12px; text-align: left; transition: background .15s ease; }
  .groupingMenu button:hover, .groupingMenu button.active { background: var(--color-accent-soft); }
  @keyframes groupingIn { from { opacity: 0; transform: translateY(-5px); } }
  @media (pointer: coarse) { .groupingButton { min-height: 44px; } .groupingMenu button { min-height: 44px; } }
  @media (prefers-reduced-motion: reduce) { .groupingMenu { animation: none; } .groupingMenu button { transition: none; } }
  .bookColumns, .level { display: grid; grid-template-columns: 1.1fr 1fr 1fr; align-items: center; text-align: right; gap: 8px; }
  .bookColumns { padding: 0 16px 10px; font-size: 12px; color: var(--color-text-soft); }
  .bookColumns > span:first-child, .level > span:first-child { text-align: left; }
  .bookColumns small { display: block; font-family: var(--font-sans); font-size: 12px; opacity: .7; margin-top: 3px; }
  .level { font-weight: 500; font-style: normal; position: relative; isolation: isolate; width: 100%; height: 28px; min-height: 28px; padding: 0 16px; font-family: var(--font-sans); font-size: 12px; font-variant-numeric: tabular-nums; }
  .level::before { content: ""; position: absolute; z-index: -1; inset: 1px 0 1px auto; width: var(--depth); background: var(--otc-buy-soft); }
  .ask::before { background: var(--otc-sell-soft); }
  .bid { color: var(--otc-buy); }
  .ask { color: var(--otc-sell); }
  .level:hover, .level.selected { background: var(--color-panel); outline: 1px solid var(--color-border); outline-offset: -1px; }
  .spread { display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 4px; margin: 7px 0; padding: 9px 16px; border-block: 1px solid var(--color-border); }
  .spread strong { color: var(--otc-buy); font-family: var(--font-sans); font-size: 16px; font-weight: 500; }
  .spread strong > span { padding-left: 6px; font-family: var(--font-sans); font-size: 12px; }
  .spread > span { font-size: 12px; color: var(--color-text-soft); }
  .spread small { font-size: 12px; }
  .pressure { padding: 13px 16px 16px; }
  .pressureLabels { display: flex; justify-content: space-between; font-family: var(--font-sans); font-size: 12px; margin-bottom: 6px; color: var(--otc-buy); }
  .pressureLabels > span:last-child { color: var(--otc-sell); }
  .pressureBar { height: 3px; border-radius: 3px; overflow: hidden; background: var(--otc-sell); }
  .pressureBar > span { display: block; height: 100%; background: var(--otc-buy); border-right: 3px solid var(--color-paper); }
  .oneSide .level { height: 39px; }
  @media (max-width: 1100px) { .level { font-size: 12px; height: 24px; } }
  @media (pointer: coarse) { .level { min-height: 44px; } .bookFilters button { width: 33px; height: 30px; } }
</style>
