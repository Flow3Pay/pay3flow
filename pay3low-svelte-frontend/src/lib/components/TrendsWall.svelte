<script lang="ts">
  import { onMount, onDestroy, tick } from "svelte";
  import { SvelteFlow, Controls, Panel, type Node as FlowNode } from "@xyflow/svelte";
  import "@xyflow/svelte/dist/style.css";
  import TrendCanvasBackground from "$lib/components/TrendCanvasBackground.svelte";
  import TrendBrickNode, { type TrendBrickData } from "$lib/components/TrendBrickNode.svelte";
  import { locale } from "$lib/i18n";
  import { assetIcon, networkIcon, venueIcon } from "$lib/icons";
  import { aggregateTrends, layoutWall, trendSwapHref, type TrendEvent, type TrendItem, type TrendMetric, type TrendPeriod } from "$lib/trends/model";
  import { readTrendActivity, TREND_STORAGE_KEY, TREND_ACTIVITY_EVENT } from "$lib/trends/activity";
  import { demoTrendActivity } from "$lib/trends/demo";
  import { trendsCopy, venueNames, networkNames } from "$lib/trends/copy";

  let mode: "demo" | "personal" = "demo";
  let metric: TrendMetric = "searches";
  let period: TrendPeriod = "1w";
  let query = "";
  let now = Date.now();
  let events: TrendEvent[] = [];
  let ready = false;
  const width = 1200;
  const nodeTypes = { trend: TrendBrickNode };
  let wallHost: HTMLDivElement;
  let measured: Record<string, number> = {};
  let measureVersion = 0;
  let brickHeight = 41;
  let detail: HTMLDivElement;
  let active: TrendItem | null = null;
  let activeAnchor: HTMLElement | null = null;
  let pinned = false;
  let suppressFocusOpen = false;
  let hoverSuppressedUntil = 0;
  let cardLeft = 0;
  let cardTop = 0;
  let closeTimer: ReturnType<typeof setTimeout> | undefined;
  const venueName = (value: string) => venueNames[value] ?? value;
  const networkName = (value: string) => networkNames[value] ?? value.replace(/-/g, " ");
  $: copy = trendsCopy[$locale];
  $: example = demoTrendActivity(now);
  $: ranked = aggregateTrends(mode === "demo" ? example : events, period, metric, now);
  $: filtered = ranked.filter((item) => `${item.asset} ${networkName(item.network)} ${venueName(item.provider)}`.toLowerCase().includes(query.trim().toLowerCase()));
  $: cells = layoutWall(filtered, width, measured);
  $: if (ready) void measureBricks(filtered, $locale, width);
  $: nodes = cells.map((cell): FlowNode<TrendBrickData> => ({
    id: cell.item.id, type: "trend", position: { x: cell.left, y: cell.row * (brickHeight + 8) },
    draggable: false, selectable: false, connectable: false, focusable: false,
    data: { enterDelay: Math.min(900, ((cells.at(-1)?.row ?? 0) - cell.row) * 55 + (cell.rank % 5) * 12), item: cell.item, active: active?.id === cell.item.id, metricLabel: copy[metric],
      value: formatter.format(cell.item[metric]), networkLabel: networkName(cell.item.network) || copy.internal,
      providerLabel: venueName(cell.item.provider), showHover, showDetail, scheduleClose }
  }));
  $: formatter = new Intl.NumberFormat($locale === "ru" ? "ru-RU" : $locale === "hy" ? "hy-AM" : "en-US");
  $: if (active && !filtered.some((item) => item.id === active?.id)) closeDetail();

  async function measureBricks(_items: TrendItem[], _locale: string, _width: number) {
    const version = ++measureVersion;
    await tick();
    if (version !== measureVersion || !wallHost) return;
    const next: Record<string, number> = {};
    for (const node of wallHost.querySelectorAll<HTMLAnchorElement>(".brick")) {
      next[node.dataset.trendId!] = node.offsetWidth;
      brickHeight = node.offsetHeight;
    }
    if (JSON.stringify(next) !== JSON.stringify(measured)) measured = next;
  }

  function resetDetail() { closeDetail(); }
  function showHover(item: TrendItem, anchor: HTMLElement) {
    if (Date.now() >= hoverSuppressedUntil) showDetail(item, anchor);
  }
  function dismissDetail(restoreFocus = true) {
    // Removing the card can expose a different brick beneath the pointer.
    hoverSuppressedUntil = Date.now() + 250;
    closeDetail(restoreFocus);
  }
  function closeDetail(restoreFocus = false) {
    clearTimeout(closeTimer);
    active = null;
    pinned = false;
    if (restoreFocus) {
      suppressFocusOpen = true;
      activeAnchor?.focus({ preventScroll: true });
      suppressFocusOpen = false;
    }
  }
  async function positionCard() {
    if (!active || !activeAnchor) return;
    await tick();
    if (!active || !detail) return;
    const rect = activeAnchor.getBoundingClientRect();
    const margin = 12;
    const gap = 10;
    if (rect.bottom <= margin || rect.top >= window.innerHeight - margin) { closeDetail(); return; }
    detail.style.maxHeight = `${window.innerHeight - 2 * margin}px`;
    const above = Math.max(0, rect.top - gap - margin);
    const below = Math.max(0, window.innerHeight - rect.bottom - gap - margin);
    const naturalHeight = detail.offsetHeight;
    const placeAbove = naturalHeight <= above || (naturalHeight > below && above >= below);
    // Fit the card in the available space without moving the document.
    detail.style.maxHeight = `${placeAbove ? above : below}px`;
    cardLeft = Math.max(margin, Math.min(rect.left + rect.width / 2 - detail.offsetWidth / 2, window.innerWidth - detail.offsetWidth - margin));
    cardTop = placeAbove ? rect.top - detail.offsetHeight - gap : rect.bottom + gap;
  }
  function showDetail(item: TrendItem, anchor: HTMLElement, pin = false) {
    if (suppressFocusOpen) return;
    clearTimeout(closeTimer);
    if (pinned && !pin) return;
    active = item;
    activeAnchor = anchor;
    pinned = pin;
    void positionCard();
  }
  function scheduleClose() {
    if (!pinned) closeTimer = setTimeout(() => {
      if (document.activeElement === activeAnchor || detail?.contains(document.activeElement)) return;
      closeDetail();
    }, 180);
  }
  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape" && active) { event.preventDefault(); dismissDetail(); }
  }
  function onPointerdown(event: PointerEvent) {
    const target = event.target as Node;
    if (active && !detail?.contains(target) && !activeAnchor?.contains(target)) closeDetail();
  }
  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return { destroy() { node.remove(); } };
  }
  onMount(() => {
    const reload = () => { now = Date.now(); events = readTrendActivity(now); closeDetail(); };
    reload();
    ready = true;
    void document.fonts.ready.then(() => measureBricks(filtered, $locale, width));
    const observer = new ResizeObserver(() => { void measureBricks(filtered, $locale, width); closeDetail(); });
    observer.observe(wallHost);
    const storage = (event: StorageEvent) => { if (event.key === TREND_STORAGE_KEY || event.key === null) reload(); };
    const timer = setInterval(reload, 60000);
    window.addEventListener(TREND_ACTIVITY_EVENT, reload);
    window.addEventListener("storage", storage);
    return () => { observer.disconnect(); clearInterval(timer); window.removeEventListener(TREND_ACTIVITY_EVENT, reload); window.removeEventListener("storage", storage); };
  });
  onDestroy(() => clearTimeout(closeTimer));
</script>

<svelte:window on:keydown={onKeydown} on:pointerdown={onPointerdown} on:scroll={() => { if (active) void positionCard(); }} on:resize={() => { if (active) void positionCard(); }} />

<section class="trends" data-testid="trends-workspace" aria-busy={!ready}>
  <div class="wallHost" bind:this={wallHost}>
    {#if ready}
      <SvelteFlow {nodes} {nodeTypes} proOptions={{ hideAttribution: true }} fitView fitViewOptions={{ padding: 0.25, maxZoom: 1, minZoom: 0.2 }} minZoom={0.15} maxZoom={2.5} nodesDraggable={false} nodesConnectable={false} elementsSelectable={false} zoomOnDoubleClick={false} onmovestart={(event) => { if (event) closeDetail(); }} onmove={(event) => { if (event) closeDetail(); else if (active) void positionCard(); }} ariaLabelConfig={{ "controls.zoomIn.ariaLabel": $locale === "ru" ? "Приблизить" : "Zoom in", "controls.zoomOut.ariaLabel": $locale === "ru" ? "Отдалить" : "Zoom out", "controls.fitView.ariaLabel": $locale === "ru" ? "Показать всю стену" : "Fit wall" }}>
        <TrendCanvasBackground {cells} {brickHeight} />
        <Panel position="top-left" class="trendsToolbar">
          <div class="toolbar">
            <div class="segmented mode" aria-label="Trends data">
              <button type="button" class:chosen={mode === "demo"} aria-pressed={mode === "demo"} on:click={() => { mode = "demo"; resetDetail(); }}>{copy.demo}</button>
              <button type="button" class:chosen={mode === "personal"} aria-pressed={mode === "personal"} on:click={() => { mode = "personal"; resetDetail(); }}>{copy.personal}</button>
            </div>
            <label class="filter"><svg width="16" height="16" viewBox="0 0 24 24" fill="none" aria-hidden="true"><circle cx="10.5" cy="10.5" r="6.5" stroke="currentColor" stroke-width="1.8" /><path d="m16 16 4 4" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" /></svg><input aria-label={copy.filter} placeholder={copy.filter} bind:value={query} on:input={resetDetail} />{#if query}<button type="button" aria-label={copy.clear} on:click={() => query = ""}>×</button>{/if}</label>
            <div class="segmented period" aria-label="Trends period">{#each [["1d", copy.day], ["1w", copy.week], ["1m", copy.month]] as [value, label]}<button type="button" class:chosen={period === value} aria-pressed={period === value} on:click={() => { period = value as TrendPeriod; resetDetail(); }}>{label}</button>{/each}</div>
            <div class="segmented metric"><button type="button" class:chosen={metric === "searches"} aria-pressed={metric === "searches"} on:click={() => { metric = "searches"; resetDetail(); }}>{copy.searches}</button><button type="button" class:chosen={metric === "appearances"} aria-pressed={metric === "appearances"} on:click={() => { metric = "appearances"; resetDetail(); }}>{copy.appearances}</button></div>
          </div>
        </Panel>
        <Controls showLock={false} fitViewOptions={{ padding: 0.25, maxZoom: 1 }} buttonBgColor="var(--color-paper)" buttonColor="var(--color-text)" buttonBgColorHover="var(--color-panel)" buttonBorderColor="var(--color-border)" />
      </SvelteFlow>
      {#if !filtered.length}
        <div class="empty"><h2>{query ? copy.noMatch : copy.empty}</h2>{#if query}<button type="button" class="primaryLink" on:click={() => query = ""}>{copy.clear} ↗</button>{:else}<a class="primaryLink" href="/#/swap">{copy.start} ↗</a>{/if}</div>
      {/if}
    {/if}
  </div>
</section>

{#if active}
  <div class="trendDetails" id="trend-details" bind:this={detail} use:portal role="region" aria-label={copy.details} style:left={`${cardLeft}px`} style:top={`${cardTop}px`} on:pointerenter={() => clearTimeout(closeTimer)} on:pointerleave={scheduleClose} on:focusin={() => clearTimeout(closeTimer)} on:focusout={(event) => { if (!detail.contains(event.relatedTarget as Node)) scheduleClose(); }}>
    <div class="detailHeader">
      <span class="avatar"><img src={assetIcon(active.asset)} alt="" width="40" height="40" /><img class="avatarNetwork" src={networkIcon(active.network)} alt="" width="14" height="14" /></span>
      <span class="detailIdentity"><strong>{active.asset}</strong><small>{networkName(active.network) || copy.internal}{#if active.provider}<span class="venueBadge"><img src={venueIcon(active.provider)} alt="" width="12" height="12" />{venueName(active.provider)}</span>{/if}</small></span>
    </div>
    <div class="detailValue"><strong>{formatter.format(active[metric])}</strong><span>{copy[metric]} <small>· {period === "1d" ? copy.day : period === "1w" ? copy.week : copy.month}</small></span></div>
    <div class="detailMetrics">
      <div><small>{copy.searches}</small><strong>{formatter.format(active.searches)}</strong></div>
      <div><small>{copy.appearances}</small><strong>{formatter.format(active.appearances)}</strong></div>
      <div><small>{copy.rank}</small><strong>#{ranked.findIndex((item) => item.id === active?.id) + 1}</strong></div>
    </div>
    <a class="explore" href={trendSwapHref(active)}>{copy.explore}<span aria-hidden="true">↗</span></a>
  </div>
{/if}

<style>
  .trends, .wallHost { position: relative; width: 100%; height: 100%; min-height: 0; }
  .wallHost { --wall-gutter: max(var(--page-gutter), calc((100% - var(--layout-width)) / 2)); }
  .wallHost :global(.trendsToolbar), .wallHost :global(.svelte-flow__controls) { left: var(--wall-gutter); margin: 0; }
  .wallHost :global(.trendsToolbar) { top: 16px; right: var(--wall-gutter); }
  .toolbar { display: flex; align-items: center; flex-wrap: wrap; gap: 10px; width: fit-content; max-width: 100%; }
  .segmented { display: inline-flex; flex: 0 0 auto; padding: 4px; gap: 3px; border: 1px solid var(--color-border); border-radius: 9px; background: var(--color-paper); box-shadow: 0 2px 8px #00000008; }
  .segmented button { min-height: 34px; padding: 0 13px; color: var(--color-text-soft); border-radius: 6px; font-size: 11px; font-weight: 750; }
  .segmented button:hover { color: var(--color-text); background: var(--color-panel-soft); }
  .segmented button.chosen { color: var(--color-text); background: var(--color-panel); }
  .period button { font-family: var(--font-mono); }
  .filter { display: flex; align-items: center; gap: 10px; width: 280px; min-width: 0; padding: 0 13px; min-height: 43px; border: 1px solid var(--color-border); border-radius: 9px; background: var(--color-paper); color: var(--color-text-soft); }
  .filter svg { flex-shrink: 0; }
  .filter input { width: 100%; min-width: 0; padding: 10px 0; border: 0; background: transparent; outline: none; font-size: 11px; }
  .filter:focus-within { outline: 2px solid var(--color-focus); outline-offset: 2px; }
  .filter input::placeholder { color: var(--color-text-soft); }
  .filter button { min-width: 26px; height: 30px; font-size: 20px; }
  .wallHost :global(.svelte-flow__node-trend) { width: max-content; }
  .wallHost :global(.svelte-flow__node-trend:has(.brick.active)), .wallHost :global(.svelte-flow__node-trend:hover) { z-index: 2 !important; }
  .wallHost :global(.svelte-flow__controls) { bottom: var(--page-gutter); border: 1px solid var(--color-border); border-radius: 8px; overflow: hidden; }
  .wallHost :global(.svelte-flow__controls-button) { width: 44px; height: 44px; background: var(--color-paper); color: var(--color-text); border-color: var(--color-border); }
  .wallHost :global(.svelte-flow__controls-button:hover) { background: var(--color-panel); }
  .empty { position: absolute; inset: 160px 16px 90px; display: grid; justify-items: center; align-content: center; gap: 20px; text-align: center; pointer-events: none; }
  .empty h2 { font-size: 20px; }
  .primaryLink { pointer-events: auto; border: 1px solid var(--color-border); border-radius: 7px; padding: 11px 17px; background: var(--color-accent); color: #182000; font-size: 12px; font-weight: 750; }
  .trendDetails { position: fixed; z-index: 200; width: 272px; max-width: calc(100vw - 24px); max-height: calc(100dvh - 24px); overflow: auto; padding: 14px; border: 1px solid var(--color-border-strong); border-radius: 7px; background: var(--color-paper); color: var(--color-text); box-shadow: 0 10px 32px #00000024; animation: detailIn .16s ease-out; }
  .detailHeader { display: flex; align-items: center; gap: 10px; }
  .avatar { flex: 0 0 40px; position: relative; width: 40px; height: 40px; }
  .avatar > img { border-radius: 50%; object-fit: contain; }
  .avatar .avatarNetwork { position: absolute; right: -2px; bottom: -2px; border: 2px solid var(--color-paper); box-sizing: content-box; }
  .detailIdentity { min-width: 0; }
  .detailHeader strong { display: block; font-size: 14px; font-weight: 800; }
  .detailHeader small { display: flex; align-items: center; flex-wrap: wrap; gap: 5px; margin-top: 4px; color: var(--color-text-soft); font-size: 10px; }
  .venueBadge { display: inline-flex; align-items: center; gap: 3px; padding: 2px 4px; border-radius: 5px; background: var(--color-panel); color: var(--color-text); font-size: 9px; }
  .venueBadge img { object-fit: contain; border-radius: 50%; }
  .detailValue { display: flex; align-items: baseline; gap: 9px; margin: 14px 0 12px; }
  .detailValue > strong { font-family: var(--font-mono); font-size: 25px; line-height: 1; font-weight: 750; }
  .detailValue > span { color: var(--color-accent-text); font-size: 11px; font-weight: 650; }
  .detailValue small { color: var(--color-text-soft); font-size: 9px; font-weight: 400; }
  .detailMetrics { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 7px; }
  .detailMetrics > div { min-width: 0; padding: 9px 8px; border-radius: 5px; background: var(--color-panel); }
  .detailMetrics small { display: block; color: var(--color-text-soft); font-size: 9px; line-height: 1.4; }
  .detailMetrics strong { display: block; margin-top: 3px; font-family: var(--font-mono); font-size: 12px; line-height: 1.3; font-weight: 700; }
  .explore { display: flex; align-items: center; gap: 7px; width: fit-content; margin-top: 11px; padding: 2px 0; color: var(--color-text-soft); font-size: 11px; }
  .explore:hover, .explore:focus-visible { color: var(--color-accent-text); }
  @keyframes detailIn { from { opacity: 0; transform: translateY(5px) scale(.98); } to { opacity: 1; transform: translateY(0) scale(1); } }
  @media (max-width: 640px) {
    .toolbar { gap: 8px; }
    .segmented button { padding-inline: 9px; min-height: 36px; }
    .filter { order: 4; width: 100%; }
    .trendDetails { width: min(272px, calc(100vw - 24px)); }
    .explore { min-height: 44px; }
  }
  @media (prefers-reduced-motion: reduce) { .trendDetails { animation: none; } }
</style>
