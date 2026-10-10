<script lang="ts">
  import { onMount } from "svelte";
  import { fade, slide } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { fetchNetworks } from "$lib/networks";
  import { locale } from "$lib/i18n";
  import { otcCopy } from "$lib/otc/copy";
  import { demoSnapshot, formatAmount, formatPrice, marketFromHash, markets, otcFallbackNetworks, type OtcMarket, type DemoOrder, type OrderDraft, type OrderSide, type OrderType } from "$lib/otc/model";
  import OtcPanel from "./OtcPanel.svelte";
  import OtcPanelToggle from "./OtcPanelToggle.svelte";
  import OtcChart from "./OtcChart.svelte";
  import OtcOrderbook from "./OtcOrderbook.svelte";
  import OtcBridge from "./OtcBridge.svelte";
  import OtcOrderReview from "./OtcOrderReview.svelte";
  let market = markets[0];
  let marketId = market.id;
  let bridgeOpen = true, bookOpen = true, activityOpen = true;
  let networks = otcFallbackNetworks;
  let sendNetworkId = "ethereum", receiveNetworkId = "bitcoin";
  let motionDuration = 320;
  let side: OrderSide = "buy";
  let orderType: OrderType = "limit";
  let price = String(market.price);
  let amount = "1000";
  let previousSide: OrderSide = side;
  let previousMarket = market.id;
  $: if (side !== previousSide || market.id !== previousMarket) {
    previousSide = side; previousMarket = market.id;
    amount = side === "buy" ? "1000" : (1000 / market.price).toFixed(market.amountDecimals);
  }
  function panelTransition(node: Element, options: { duration: number }) {
    const transition = window.matchMedia("(max-width: 980px)").matches ? slide : fade;
    return transition(node, { ...options, easing: cubicOut });
  }
  let tab: "trades" | "orders" | "history" = "trades";
  let orders: DemoOrder[] = [];
  let reviewDraft: OrderDraft | null = null;
  let notification = "";
  let notificationTimer: ReturnType<typeof setTimeout> | undefined;
  const storageKey = "pay3flow.otc.demo-orders.v1";
  const layoutKey = "pay3flow.otc.panels.v1";
  const settlementKey = "pay3flow.otc.settlement.v1";
  let mounted = false;
  let chartColumn: HTMLDivElement;
  let toggleOffset = 310;
  $: if (mounted) {
    try { localStorage.setItem(layoutKey, JSON.stringify({ bridgeOpen, bookOpen, activityOpen })); } catch { /* Layout still works without storage. */ }
    try { localStorage.setItem(settlementKey, JSON.stringify({ marketId: market.id, side, sendNetworkId, receiveNetworkId })); } catch { /* Selection still works without storage. */ }
  }
  $: copy = otcCopy($locale);
  $: snapshot = demoSnapshot(market);
  $: marketOrders = orders.filter((order) => order.marketId === market.id);
  $: openOrders = marketOrders.filter((order) => order.status === "open");
  $: history = marketOrders.filter((order) => order.status !== "open");
  $: high = market.price * 1.014;
  $: low = market.price * .959;
  function setMarket(next: OtcMarket, resetSide = true) {
    if (next.id === market.id) return;
    market = next; marketId = next.id; price = String(next.price); if (resetSide) chooseSide("buy"); orderType = "limit"; reviewDraft = null;
  }
  function selectMarket() {
    const next = markets.find((item) => item.id === marketId);
    if (!next) return;
    setMarket(next);
    const url = new URL(location.href);
    url.hash = `/otc?market=${next.id}`;
    historyReplace(url);
  }
  function historyReplace(url: URL) { window.history.replaceState(window.history.state, "", url); window.dispatchEvent(new HashChangeEvent("hashchange")); }
  function selectBridgeAsset(asset: string, field: "send" | "receive", network: (typeof networks)[number]) {
    const next = asset === "USDT" ? market : markets.find((item) => item.base === asset);
    if (!next) return;
    setMarket(next, false);
    chooseSide(field === "send" ? asset === "USDT" ? "buy" : "sell" : asset === "USDT" ? "sell" : "buy");
    if (field === "send") sendNetworkId = network.id;
    else receiveNetworkId = network.id;
    const url = new URL(location.href);
    url.hash = `/otc?market=${next.id}`;
    // This selection already set the side and network; no hash event is needed.
    window.history.replaceState(window.history.state, "", url);
  }
  function chooseSide(next: OrderSide) { if (next !== side) { [sendNetworkId, receiveNetworkId] = [receiveNetworkId, sendNetworkId]; side = next; } }
  function selectPrice(nextSide: OrderSide, nextPrice: number) { chooseSide(nextSide); price = nextPrice.toFixed(market.priceDecimals); orderType = "limit"; bridgeOpen = true; }
  function notify(message: string) {
    notification = message;
    clearTimeout(notificationTimer);
    notificationTimer = setTimeout(() => notification = "", 4500);
  }
  function persistOrders() {
    try { sessionStorage.setItem(storageKey, JSON.stringify(orders.slice(0, 100))); } catch { /* Orders still work within the current page. */ }
  }
  function confirmOrder() {
    if (!reviewDraft) return;
    const draft = reviewDraft;
    const order: DemoOrder = { id: crypto.randomUUID(), marketId: draft.marketId, side: draft.side, type: draft.type, amount: draft.amount, price: draft.price, sendNetwork: draft.sendNetwork, receiveNetwork: draft.receiveNetwork, createdAt: Date.now(), status: draft.type === "market" ? "simulated" : "open" };
    orders = [order, ...orders].slice(0, 100); persistOrders(); reviewDraft = null; activityOpen = true;
    tab = order.type === "market" ? "history" : "orders";
    notify(order.type === "market" ? copy.orderSimulated : copy.orderCreated);
  }
  function cancelOrder(id: string) { orders = orders.map((order) => order.id === id ? { ...order, status: "cancelled" } : order); persistOrders(); notify(copy.orderCancelled); }
  function navigateTab(event: KeyboardEvent) {
    const tabs = ["trades", "orders", "history"] as const;
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
    event.preventDefault();
    const next = event.key === "Home" ? 0 : event.key === "End" ? 2 : (tabs.indexOf(tab) + (event.key === "ArrowRight" ? 1 : -1) + 3) % 3;
    tab = tabs[next];
    document.getElementById(`otc-${tab}-tab`)?.focus();
  }
  function timeLabel(time: number) { return new Date(time).toLocaleTimeString("en-GB", { hour12: false, timeZone: "UTC" }); }
  function validOrder(value: unknown): value is DemoOrder {
    if (!value || typeof value !== "object") return false;
    const order = value as DemoOrder;
    return typeof order.id === "string" && markets.some((item) => item.id === order.marketId) && ["buy", "sell"].includes(order.side) && ["limit", "market"].includes(order.type) && ["open", "cancelled", "simulated"].includes(order.status) && [order.price, order.amount, order.createdAt].every((number) => typeof number === "number" && Number.isFinite(number) && number > 0);
  }
  onMount(() => {
    let disposed = false;
    const media = matchMedia("(prefers-reduced-motion: reduce)");
    const motionChanged = () => motionDuration = media.matches ? 0 : 320;
    motionChanged(); media.addEventListener("change", motionChanged);
    fetchNetworks().then((items) => { if (!disposed && items.length) networks = items; }).catch(() => {});
    setMarket(marketFromHash(location.hash));
    try {
      const saved = JSON.parse(localStorage.getItem(settlementKey) ?? "null");
      if (saved?.marketId === market.id) {
        if (["buy", "sell"].includes(saved.side)) side = saved.side;
        if (typeof saved.sendNetworkId === "string") sendNetworkId = saved.sendNetworkId;
        if (typeof saved.receiveNetworkId === "string") receiveNetworkId = saved.receiveNetworkId;
      }
    } catch { /* Ignore damaged selection storage. */ }
    try {
      const saved = JSON.parse(localStorage.getItem(layoutKey) ?? "null");
      if (saved && typeof saved === "object") {
        if (typeof saved.bridgeOpen === "boolean") bridgeOpen = saved.bridgeOpen;
        if (typeof saved.bookOpen === "boolean") bookOpen = saved.bookOpen;
        if (typeof saved.activityOpen === "boolean") activityOpen = saved.activityOpen;
      }
    } catch { /* Ignore damaged layout storage. */ }
    const positionToggles = () => {
      const bounds = chartColumn.getBoundingClientRect();
      toggleOffset = Math.max(0, bounds.height / 2 - 21);
    };
    const observer = new ResizeObserver(positionToggles);
    observer.observe(chartColumn);
    positionToggles();
    mounted = true;
    try { const stored: unknown = JSON.parse(sessionStorage.getItem(storageKey) ?? "[]"); if (Array.isArray(stored)) orders = stored.filter(validOrder).slice(0, 100); } catch { /* Ignore damaged demo storage. */ }
    const hashChanged = () => setMarket(marketFromHash(location.hash));
    window.addEventListener("hashchange", hashChanged);
    return () => { disposed = true; media.removeEventListener("change", motionChanged); observer.disconnect(); clearTimeout(notificationTimer); window.removeEventListener("hashchange", hashChanged); };
  });
</script>
<section class="otcWorkspace" data-testid="otc-workspace">
  <div class="pageIntro"><div><h1>{copy.titleLead}<span class="titleAccent">{copy.titleAccent}</span></h1><p>{copy.subtitle}</p></div></div>
  <div class="marketStrip">
    <div class="marketSelector"><img src={market.icon} width="38" height="38" alt="" /><div><div class="pairSelect"><select aria-label={copy.marketPicker} bind:value={marketId} on:change={selectMarket}>{#each markets as item}<option value={item.id}>{item.base} / {item.quote}</option>{/each}</select><svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="m4 6 4 4 4-4" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" /></svg></div><span>{market.name} <span class="marketTag">OTC</span></span></div></div>
    <div class="marketStat last"><span>{copy.lastPrice}</span><strong>{formatPrice(market.price, market)} <small>{market.quote}</small></strong></div>
    <div class="marketStat"><span>{copy.change}</span><strong class="positive">+{market.change.toFixed(2)}% <span aria-hidden="true">↗</span></strong></div>
    <div class="marketStat secondaryStat"><span>{copy.high}</span><strong>{formatPrice(high, market)}</strong></div>
    <div class="marketStat secondaryStat"><span>{copy.low}</span><strong>{formatPrice(low, market)}</strong></div>
    <div class="marketStat volumeStat"><span>{copy.volume}</span><strong>{market.volume.toLocaleString("en-US")} <small>{market.quote}</small></strong></div>
  </div>
  <div class="otcGrid" class:bridgeClosed={!bridgeOpen} class:bookClosed={!bookOpen} style:--panel-toggle-offset={`${toggleOffset}px`}>
    {#if bridgeOpen}<div class="bridgeColumn" id="otc-bridge-column" transition:panelTransition={{ duration: motionDuration }}><OtcPanel id="otc-bridge" title={copy.bridge} collapsible={false}><span slot="actions" class="panelMeta">OTC</span><OtcBridge {market} {snapshot} {networks} bind:sendNetworkId bind:receiveNetworkId onSelectAsset={selectBridgeAsset} onSideChange={chooseSide} bind:amount bind:side bind:type={orderType} bind:price onReview={(draft) => reviewDraft = draft} /></OtcPanel></div>{/if}
    <div class="bridgeToggle"><OtcPanelToggle title={copy.bridge} controls="otc-bridge-column" bind:expanded={bridgeOpen} /></div>
    <div class="chartColumn" id="otc-chart-column" bind:this={chartColumn}><OtcPanel id="otc-chart" title={copy.chart} collapsible={false}>{#key market.id}<OtcChart {market} />{/key}</OtcPanel></div>
    <div class="bookToggle"><OtcPanelToggle title={copy.book} controls="otc-book-column" bind:expanded={bookOpen} direction="right" /></div>
    {#if bookOpen}<div class="bookColumn" id="otc-book-column" transition:panelTransition={{ duration: motionDuration }}><OtcPanel id="otc-book" title={copy.book} collapsible={false}>{#key market.id}<OtcOrderbook {market} {snapshot} selectedPrice={orderType === "limit" ? Number(price) : null} onSelect={selectPrice} />{/key}</OtcPanel></div>{/if}
  </div>
  <div class="activityToggle"><OtcPanelToggle title={copy.activity} controls="otc-activity-section" bind:expanded={activityOpen} vertical /></div>
  {#if activityOpen}<div class="activitySection" id="otc-activity-section" transition:slide={{ duration: motionDuration, easing: cubicOut }}><OtcPanel id="otc-activity" title={copy.activity} collapsible={false}><span slot="actions" class="panelMeta">{market.base}/{market.quote}</span>
    <div class="activityTabs" role="tablist" aria-label={copy.activity}><button type="button" role="tab" id="otc-trades-tab" aria-selected={tab === "trades"} tabindex={tab === "trades" ? 0 : -1} on:keydown={navigateTab} aria-controls="otc-trades-content" class:active={tab === "trades"} on:click={() => tab = "trades"}>{copy.book}</button><button type="button" role="tab" id="otc-orders-tab" aria-selected={tab === "orders"} tabindex={tab === "orders" ? 0 : -1} on:keydown={navigateTab} aria-controls="otc-orders-content" class:active={tab === "orders"} on:click={() => tab = "orders"}>{copy.myOrders}{#if openOrders.length}<span class="orderCount">{openOrders.length}</span>{/if}</button><button type="button" role="tab" id="otc-history-tab" aria-selected={tab === "history"} tabindex={tab === "history" ? 0 : -1} on:keydown={navigateTab} aria-controls="otc-history-content" class:active={tab === "history"} on:click={() => tab = "history"}>{copy.history}</button></div>
    <div class="activityContent" role="tabpanel" id={`otc-${tab === "trades" ? "trades" : tab === "orders" ? "orders" : "history"}-content`} aria-labelledby={`otc-${tab === "trades" ? "trades" : tab === "orders" ? "orders" : "history"}-tab`} tabindex="0">
      {#if tab === "trades"}
        <div class="splitBooks">
          <div class="buyBook" data-testid="otc-buy-book"><OtcPanel id="otc-buy-book" title={copy.bids} collapsible={false}>{#key market.id}<OtcOrderbook {market} {snapshot} sideOnly="buy" showTools={false} selectedPrice={orderType === "limit" ? Number(price) : null} onSelect={selectPrice} />{/key}</OtcPanel></div>
          <div class="sellBook" data-testid="otc-sell-book"><OtcPanel id="otc-sell-book" title={copy.asks} collapsible={false}>{#key market.id}<OtcOrderbook {market} {snapshot} sideOnly="sell" showTools={false} selectedPrice={orderType === "limit" ? Number(price) : null} onSelect={selectPrice} />{/key}</OtcPanel></div>
        </div>
      {:else if (tab === "orders" ? openOrders : history).length}
        <div class="tableScroll"><table><thead><tr><th>{copy.time} <small>UTC</small></th><th>{copy.side}</th><th>{copy.price} <small>{market.quote}</small></th><th>{copy.amount} <small>{market.base}</small></th><th>{copy.status}</th>{#if tab === "orders"}<th>{copy.action}</th>{/if}</tr></thead><tbody>{#each (tab === "orders" ? openOrders : history) as order}<tr><td class="muted">{timeLabel(order.createdAt)}</td><td><span class="sidePill" class:sell={order.side === "sell"}>{order.side === "buy" ? copy.buy : copy.sell}</span></td><td>{formatPrice(order.price, market)}</td><td>{formatAmount(order.amount, market)}</td><td><span class="statusPill" class:isOpen={order.status === "open"}>{order.status === "open" ? copy.open : order.status === "cancelled" ? copy.cancelled : copy.simulated}</span></td>{#if tab === "orders"}<td><button type="button" class="cancelOrder" on:click={() => cancelOrder(order.id)}>{copy.cancel}</button></td>{/if}</tr>{/each}</tbody></table></div>
      {:else}
        <div class="emptyState"><div class="emptyIcon"><svg width="27" height="27" viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="M5 4h14v16H5V4Zm4 5h6m-6 4h6m-6 4h3" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" /></svg></div><h3>{tab === "orders" ? copy.noOrders : copy.noHistory}</h3><p>{tab === "orders" ? copy.noOrdersHint : copy.noHistoryHint}</p>{#if tab === "orders"}<button type="button" on:click={() => { bridgeOpen = true; document.getElementById("otc-send")?.focus(); }}>{copy.create}<span aria-hidden="true">↗</span></button>{/if}</div>
      {/if}
    </div>
  </OtcPanel></div>{/if}
</section>
{#if notification}<div class="orderNotification" role="status"><span aria-hidden="true">✓</span>{notification}<button type="button" aria-label={copy.close} on:click={() => notification = ""}>×</button></div>{/if}
{#if reviewDraft}<OtcOrderReview draft={reviewDraft} {market} onClose={() => reviewDraft = null} onConfirm={confirmOrder} />{/if}
<style>
  .otcWorkspace { --otc-buy: var(--color-good); --otc-sell: var(--color-danger); --otc-buy-soft: color-mix(in srgb, var(--color-good) 13%, transparent); --otc-sell-soft: color-mix(in srgb, var(--color-danger) 9%, transparent); width: min(var(--layout-width), calc(100% - 2 * var(--page-gutter))); margin: 0 auto; padding: 34px 0 20px; }
  .pageIntro { position: relative; display: grid; justify-items: center; gap: 14px; margin: 0 auto 26px; text-align: center; }
  h1 { margin: 0 auto; max-width: 1000px; text-wrap: balance; font-size: clamp(44px, 5.5vw, 72px); font-weight: 650; letter-spacing: -.065em; line-height: .96; }
  .titleAccent { position: relative; z-index: 0; display: inline-block; }
  .titleAccent::after { position: absolute; right: -.05em; bottom: .02em; left: -.04em; z-index: -1; height: .2em; border-radius: 3px; background: var(--color-accent); content: ""; transform: rotate(-1deg); }
  .pageIntro p { max-width: 560px; margin: 12px auto 0; font-size: 16px; line-height: 1.55; color: var(--color-text-soft); }
  .marketStrip { display: flex; align-items: center; justify-content: space-between; gap: 20px; min-height: 88px; padding: 17px 22px; border: 1px solid var(--color-border); border-radius: 10px; background: var(--color-paper); margin-bottom: 24px; }
  .marketSelector { display: flex; align-items: center; gap: 12px; padding-right: 25px; border-right: 1px solid var(--color-border); }
  .marketSelector > img { border-radius: 50%; }
  .pairSelect { display: flex; align-items: center; gap: 3px; }
  .pairSelect select { appearance: none; border: 0; background: transparent; font-weight: 800; font-size: 14px; cursor: pointer; padding: 3px 0; }
  .pairSelect option { background: var(--color-paper); color: var(--color-text); }
  .pairSelect svg { pointer-events: none; }
  .marketSelector > div > span { display: flex; align-items: center; gap: 8px; color: var(--color-text-soft); font-size: 12px; margin-top: 4px; }
  .marketTag { display: inline-block; background: var(--color-panel); padding: 2px 4px; border-radius: 3px; font-family: var(--font-mono); font-size: 12px; }
  .marketStat { display: grid; gap: 8px; white-space: nowrap; }
  .marketStat > span { font-size: 12px; color: var(--color-text-soft); }
  .marketStat strong { font-family: var(--font-mono); font-weight: 400; font-size: 12px; }
  .marketStat strong small { font-size: 12px; color: var(--color-text-soft); }
  .marketStat.last strong { font-size: 18px; letter-spacing: -.05em; }
  .marketStat strong.positive { color: var(--otc-buy); }
  .otcGrid { display: grid; grid-template-columns: minmax(0, 1fr) 32px minmax(0, 1.3fr) 32px minmax(0, .85fr); align-items: start; transition: grid-template-columns .38s cubic-bezier(.22, 1, .36, 1), transform .38s cubic-bezier(.22, 1, .36, 1); }
  .bridgeColumn { grid-column: 1; grid-row: 1; }
  .chartColumn { grid-column: 3; grid-row: 1; }
  .bookColumn { grid-column: 5; grid-row: 1; }
  .bridgeColumn, .chartColumn, .bookColumn { min-width: 0; }
  .bridgeColumn, .bookColumn { overflow: clip; }
  .bridgeToggle, .bookToggle { display: grid; justify-items: center; grid-row: 1; padding-top: var(--panel-toggle-offset); }
  .bridgeToggle { grid-column: 2; }
  .bookToggle { grid-column: 4; }
  .otcGrid.bridgeClosed { grid-template-columns: minmax(0, 0fr) 32px minmax(0, 1.3fr) 32px minmax(0, .85fr); }
  .otcGrid.bookClosed { grid-template-columns: minmax(0, 1fr) 32px minmax(0, 1.3fr) 32px minmax(0, 0fr); }
  .otcGrid.bridgeClosed.bookClosed { grid-template-columns: minmax(0, 0fr) 32px minmax(0, 1fr) 32px minmax(0, 0fr); }
  .splitBooks { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: 16px; padding: 16px; }
  .buyBook, .sellBook { min-width: 0; }
  .buyBook :global(.panelTitle h2) { color: var(--otc-buy); }
  .sellBook :global(.panelTitle h2) { color: var(--otc-sell); }
  .activityToggle { display: grid; height: 42px; place-items: center; }
  .panelMeta { color: var(--color-text-faint); font-family: var(--font-mono); font-size: 12px; }
  .activitySection { min-width: 0; }
  .activityTabs { display: flex; align-items: center; gap: 23px; padding: 0 20px; border-bottom: 1px solid var(--color-border); }
  .activityTabs button { position: relative; display: flex; align-items: center; gap: 7px; min-height: 48px; color: var(--color-text-soft); font-size: 12px; white-space: nowrap; }
  .activityTabs button.active { font-weight: 800; color: var(--color-text); }
  .activityTabs button.active::after { content: ""; position: absolute; left: 0; right: 0; bottom: -1px; height: 2px; border-radius: 2px; background: var(--color-accent-strong); }
  .orderCount { display: grid; place-items: center; min-width: 16px; height: 16px; padding: 0 4px; border-radius: 4px; background: var(--color-accent-soft); font-family: var(--font-mono); font-size: 12px; }
  .activityContent { min-height: 237px; }
  .tableScroll { overflow-x: auto; }
  table { width: 100%; border-collapse: collapse; text-align: right; font-family: var(--font-sans); font-variant-numeric: tabular-nums; font-size: 12px; white-space: nowrap; }
  th { font-family: var(--font-sans); font-size: 12px; font-weight: 500; color: var(--color-text-soft); padding: 13px 22px; border-bottom: 1px solid var(--color-border); }
  th small { font-family: var(--font-mono); font-size: 12px; margin-left: 4px; }
  td { padding: 8px 22px; }
  td:first-child, th:first-child, td:nth-child(2), th:nth-child(2) { text-align: left; }
  tbody tr:nth-child(even) { background: var(--color-panel-soft); }
  tbody tr:hover { background: var(--color-panel); }
  .muted { color: var(--color-text-soft); }
  .buyText { color: var(--otc-buy); }
  .sellText { color: var(--otc-sell); }
  .sidePill { display: inline-block; min-width: 57px; padding: 4px 7px; border-radius: 4px; text-align: center; background: var(--otc-buy-soft); color: var(--otc-buy); font-family: var(--font-sans); font-size: 12px; }
  .sidePill.sell { background: var(--otc-sell-soft); color: var(--otc-sell); }
  .statusPill { color: var(--color-text-soft); font-family: var(--font-sans); font-size: 12px; }
  .statusPill.isOpen { color: var(--otc-buy); }
  .cancelOrder { padding: 5px 8px; border: 1px solid var(--color-border); border-radius: 5px; font-family: var(--font-sans); font-size: 12px; }
  .cancelOrder:hover { border-color: var(--otc-sell); color: var(--otc-sell); }
  .emptyState { display: grid; justify-items: center; padding: 28px 16px; text-align: center; }
  .emptyIcon { display: grid; width: 47px; height: 47px; place-items: center; margin-bottom: 12px; border: 1px solid var(--color-border); border-radius: 12px; background: var(--color-panel); color: var(--color-text-soft); }
  .emptyState h3 { font-size: 14px; font-weight: 700; }
  .emptyState p { margin-top: 7px; font-size: 12px; color: var(--color-text-soft); }
  .emptyState button { display: flex; align-items: center; gap: 16px; margin-top: 16px; padding: 9px 13px; border: 1px solid var(--color-border); border-radius: 7px; font-size: 12px; font-weight: 700; }
  .emptyState button:hover { background: var(--color-accent-soft); }
  .orderNotification { position: fixed; bottom: 24px; left: 50%; z-index: 100; display: flex; align-items: center; gap: 12px; width: max-content; max-width: calc(100vw - 32px); padding: 14px 17px; background: var(--color-paper); border: 1px solid var(--color-border); border-radius: 12px; box-shadow: 0 8px 40px #0002; transform: translateX(-50%); font-size: 12px; }
  .orderNotification > span { color: var(--color-accent-text); }
  .orderNotification button { margin-left: 8px; font-size: 18px; }
  [hidden] { display: none !important; }
  @media (max-width: 1200px) and (min-width: 981px) { .otcGrid { grid-template-columns: minmax(0, 1fr) 32px minmax(0, 1.25fr) 32px minmax(0, .9fr); } .marketStrip { padding: 16px; gap: 15px; } .marketSelector { padding-right: 15px; } .secondaryStat { display: none; } }
  @media (max-width: 980px) {
    .otcGrid, .otcGrid.bridgeClosed, .otcGrid.bookClosed, .otcGrid.bridgeClosed.bookClosed { display: flex; width: 100%; flex-direction: column; transform: none; }
    .bridgeColumn, .chartColumn, .bookColumn { width: 100%; }
    .bridgeToggle, .bookToggle { width: 100%; height: 42px; padding: 0; align-items: center; }
    .marketStrip { flex-wrap: wrap; justify-content: flex-start; gap: 16px 28px; }
    .marketSelector { border-right: 0; flex: 1; }
    .last { flex: 1; }
    .marketStat:not(.last) { flex: 1; border-top: 1px solid var(--color-border); padding-top: 12px; }
  }
  @media (max-width: 640px) {
    .otcWorkspace { padding-top: 20px; }
    h1 { font-size: clamp(36px, 9vw, 48px); }
    .pageIntro p { font-size: 14px; }
    .secondaryStat { display: none; }
    .marketStat.last strong { font-size: 16px; }
    .activityTabs { gap: 17px; padding-inline: 14px; overflow-x: auto; }
    th, td { padding-inline: 14px; }
  }
  @media (prefers-reduced-motion: reduce) { .otcGrid { transition: none; } }
</style>
