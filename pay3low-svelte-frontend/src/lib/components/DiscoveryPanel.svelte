<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { apiUrl, wsUrl } from "$lib/api";
  import { discoveryLink, mergeDiscoveryRoutes, routeExpired, type DiscoveryMode, type DiscoveryQuery, type DiscoveryResponse, type DiscoveryRoute } from "$lib/discovery";
  import type { PaymentMethod } from "$lib/payment-methods";
  import type { CryptoNetwork } from "$lib/networks";

  export let mode: DiscoveryMode = "exchange";
  export let paymentMethods: PaymentMethod[] = [];
  export let networks: CryptoNetwork[] = [];
  let source = "ANY", target = "ANY", sourceMethod = "", targetMethod = "", sourceNetwork = "", targetNetwork = "";
  let amount = "1000", maxActions = 4, crossVenue = true;
  let exchangeMode: "all" | "p2p" | "exchanger" = "all";
  let providers = "";
  let routes: DiscoveryRoute[] = [];
  let response: DiscoveryResponse | null = null;
  let loading = false, searched = false, error = "", copied = false;
  let socket: WebSocket | undefined, controller: AbortController | undefined;
  let sequence = 0, clock = Date.now(), mounted = false;
  let lastMode = mode;
  const storageKey = "pay3flow.discovery.selection";
  $: currencies = [...new Set([...paymentMethods.map(m => m.currency), ...networks.flatMap(n => n.currencies)])].sort();
  $: sendingMethods = paymentMethods.filter(m => m.currency === source && m.kind !== "wallet" && m.kind !== "currency" && m.role !== "recipient");
  $: receivingMethods = paymentMethods.filter(m => m.currency === target && m.kind !== "wallet" && m.kind !== "currency" && m.role !== "sender");
  $: sendingNetworks = networks.filter(n => n.currencies.includes(source));
  $: receivingNetworks = networks.filter(n => n.currencies.includes(target));
  $: visibleRoutes = routes.filter(route => !routeExpired(route, clock));
  $: if (mode !== lastMode) { lastMode = mode; reset(); if (mounted) remember(); }

  function query(): DiscoveryQuery {
    return { mode, source_fiat: source, target_fiat: mode === "arbitrage" ? source : target, source_amount: amount,
      source_payment_method: sourceMethod || undefined, target_payment_method: (mode === "arbitrage" ? sourceMethod : targetMethod) || undefined,
      source_network: sourceNetwork || undefined, target_network: (mode === "arbitrage" ? sourceNetwork : targetNetwork) || undefined,
      allow_cross_venue: crossVenue, max_actions: Number(maxActions), exchange_mode: exchangeMode, sources: providers.trim() || undefined };
  }
  function cancel() { sequence++; socket?.close(); socket = undefined; controller?.abort(); controller = undefined; loading = false; }
  function reset() { cancel(); routes = []; response = null; error = ""; searched = false; }
  function changed(side?: "source" | "target") {
    if (side === "source") { sourceMethod = ""; sourceNetwork = ""; }
    if (side === "target") { targetMethod = ""; targetNetwork = ""; }
    reset(); remember();
  }
  function remember() {
    if (!mounted) return;
    try { localStorage.setItem(storageKey, JSON.stringify(query())); } catch {}
    if (window.location.hash.startsWith("#/discover")) history.replaceState(null,"",discoveryLink(query()));
  }
  function restore(value: Partial<DiscoveryQuery>) {
    source = value.source_fiat ?? "ANY"; target = value.target_fiat ?? "ANY"; amount = value.source_amount ?? "1000";
    sourceMethod = value.source_payment_method ?? ""; targetMethod = value.target_payment_method ?? "";
    sourceNetwork = value.source_network ?? ""; targetNetwork = value.target_network ?? "";
    maxActions = Number(value.max_actions ?? 4); crossVenue = value.allow_cross_venue ?? true;
    exchangeMode = value.exchange_mode ?? "all"; providers = value.sources ?? "";
  }
  onMount(() => {
    try { const saved = localStorage.getItem(storageKey); if (saved) restore(JSON.parse(saved)); } catch {}
    if (window.location.hash.startsWith("#/discover?")) {
      const p = new URLSearchParams(window.location.hash.split("?")[1]);
      restore({ source_fiat: p.get("source_fiat") ?? "ANY", target_fiat: p.get("target_fiat") ?? "ANY", source_amount: p.get("source_amount") ?? "1000",
        source_payment_method: p.get("source_payment_method") ?? undefined, target_payment_method: p.get("target_payment_method") ?? undefined,
        source_network: p.get("source_network") ?? undefined, target_network: p.get("target_network") ?? undefined,
        max_actions: Number(p.get("max_actions") ?? 4), allow_cross_venue: p.get("allow_cross_venue") !== "false",
        exchange_mode: (["all","p2p","exchanger"].includes(p.get("exchange_mode") ?? "") ? p.get("exchange_mode") : "all") as DiscoveryQuery["exchange_mode"], sources: p.get("sources") ?? undefined });
    }
    mounted = true;
    const timer = setInterval(() => clock = Date.now(),1000);
    return () => clearInterval(timer);
  });
  onDestroy(cancel);
  function apply(next: DiscoveryResponse, continuation: boolean, previous: DiscoveryRoute[]) {
    response = next;
    routes = mergeDiscoveryRoutes(continuation ? previous : [],next.routes,mode);
  }
  async function search(continuation = false) {
    const cursor = continuation ? response?.next_cursor : undefined;
    cancel(); const current = sequence;
    if (!/^\d+(\.\d+)?$/.test(amount) || !Number.isFinite(Number(amount)) || Number(amount) <= 0) { error = "Enter an amount greater than zero."; return; }
    loading = true; searched = true; error = "";
    const previous = continuation ? routes : [];
    if (!continuation) { routes = []; response = null; }
    remember();
    history.replaceState(null,"",discoveryLink(query()));
    const request = { ...query(), ...(cursor ? { cursor } : {}) };
    let received = false;
    try {
      await new Promise<void>((resolve,reject) => {
        const ws = new WebSocket(wsUrl("/ws/p2p/discovery")); socket = ws;
        const timer = setTimeout(() => { ws.close(); reject(new Error("Search timed out. Try again.")); },65_000);
        const finish = (error?: Error) => { clearTimeout(timer); ws.close(); if (error) reject(error); else resolve(); };
        ws.onopen = () => ws.send(JSON.stringify(request));
        ws.onmessage = event => {
          if (current !== sequence) return finish();
          try {
            const message = JSON.parse(event.data);
            if (message.type === "search_failed") return finish(new Error(message.error));
            if (message.response) { received = true; apply(message.response,continuation,previous); }
            if (message.type === "search_finished") finish();
          } catch { finish(new Error("The search response could not be read.")); }
        };
        ws.onerror = () => finish(new Error("Live search connection failed."));
        ws.onclose = () => { clearTimeout(timer); if (current !== sequence) resolve(); else if (received && response?.next_cursor) resolve(); else reject(new Error("Live search connection closed.")); };
      });
    } catch (cause) {
      if (current !== sequence) return;
      // HTTP fallback is only used before any streamed result, avoiding a duplicate fan-out after partial results.
      if (!received) {
        try {
          controller = new AbortController();
          const timer = setTimeout(() => controller?.abort(),65_000);
          try {
            const url = apiUrl("/api/p2p/discovery");
            for (const [key,value] of Object.entries(request)) if (value !== undefined) url.searchParams.set(key,String(value));
            const reply = await fetch(url,{ signal:controller.signal });
            if (!reply.ok) throw new Error(await reply.text());
            if (current === sequence) apply(await reply.json(),continuation,previous);
          } finally { clearTimeout(timer); }
        } catch (fallback) { if (current === sequence) error = fallback instanceof Error ? fallback.message : "Search failed. Try again."; }
      } else error = cause instanceof Error ? cause.message : "Search interrupted. Try again.";
    } finally { if (current === sequence) loading = false; }
  }
  async function share() {
    try { await navigator.clipboard.writeText(new URL(discoveryLink(query()),window.location.origin).toString()); copied = true; }
    catch { error = "Copy the page address to share these selections."; }
  }
  function money(value: number | null) { return value === null ? "Unavailable" : new Intl.NumberFormat("en-US",{style:"currency",currency:"USD",maximumFractionDigits:4}).format(value); }
  function links(route: DiscoveryRoute) {
    const candidates = [route.entry_offer?.source_url,...(route.cycle_legs ?? []).map(l => l.source_url),route.route_provider_url,route.exit_offer?.source_url];
    return [...new Set(candidates.filter((link): link is string => Boolean(link && /^https?:\/\//.test(link))))];
  }
</script>

<section class="discovery" aria-label={mode === "arbitrage" ? "Arbitrage discovery" : "Any to any exchange"}>
  <form on:submit|preventDefault={() => search()}>
    <div class="heading"><div><h2>{mode === "arbitrage" ? "Find arbitrage opportunities" : "Explore any currency and method"}</h2><p>{mode === "arbitrage" ? "Return to the same currency, network, payment method and venue." : "Compare value received per value sent across supported providers."}</p></div><button type="button" class="secondary" on:click={share}>{copied ? "Link copied" : "Share search"}</button></div>
    <div class="fields">
      <label>Starting currency<select bind:value={source} on:change={() => changed("source")}><option value="ANY">Any currency</option>{#each currencies as currency}<option value={currency}>{currency}</option>{/each}</select></label>
      <label>Sending method<select bind:value={sourceMethod} disabled={source === "ANY" || sendingNetworks.length > 0} on:change={() => changed()}><option value="">Any method</option>{#each sendingMethods as method}<option value={method.id}>{method.name}</option>{/each}</select></label>
      {#if sendingNetworks.length}<label>Starting network<select bind:value={sourceNetwork} on:change={() => changed()}><option value="">Any supported network</option>{#each sendingNetworks as network}<option value={network.id}>{network.name}</option>{/each}</select></label>{/if}
      {#if mode === "exchange"}
        <label>Receiving currency<select bind:value={target} on:change={() => changed("target")}><option value="ANY">Any currency</option>{#each currencies as currency}<option value={currency}>{currency}</option>{/each}</select></label>
        <label>Receiving method<select bind:value={targetMethod} disabled={target === "ANY" || receivingNetworks.length > 0} on:change={() => changed()}><option value="">Any method</option>{#each receivingMethods as method}<option value={method.id}>{method.name}</option>{/each}</select></label>
        {#if receivingNetworks.length}<label>Receiving network<select bind:value={targetNetwork} on:change={() => changed()}><option value="">Any supported network</option>{#each receivingNetworks as network}<option value={network.id}>{network.name}</option>{/each}</select></label>{/if}
      {/if}
      <label>Amount per starting currency<input inputmode="decimal" bind:value={amount} on:input={() => changed()} required pattern="[0-9]+(\.[0-9]+)?" /></label>
      <label>Maximum actions<select bind:value={maxActions} on:change={() => changed()}>{#each [2,3,4,5,6,7,8] as count}<option value={count}>{count} actions</option>{/each}</select></label>
      <label>Sources<select bind:value={exchangeMode} on:change={() => changed()}><option value="all">All supported sources</option><option value="p2p">P2P advertisements</option><option value="exchanger">Exchangers and swaps</option></select></label>
      <label>Provider filter<input bind:value={providers} on:change={() => changed()} placeholder="All providers, or binance,okx" /></label>
    </div>
    <label class="check"><input type="checkbox" bind:checked={crossVenue} on:change={() => changed()} />Allow intermediate transfers between venues</label>
    <p class="note">{source === "ANY" ? `The same number is searched separately in each currency. ${amount || "1000"} RUB and ${amount || "1000"} USDT represent different starting capital.` : `Starting capital is ${amount || "0"} ${source}.`} USD values are indicative. Missing costs keep profit unconfirmed.</p>
    <div class="actions"><button type="submit" disabled={loading || !currencies.length}>{loading ? "Searching…" : mode === "arbitrage" ? "Find arbitrage" : "Find exchanges"}</button>{#if loading}<button type="button" class="secondary" on:click={cancel}>Stop search</button>{/if}{#if response?.next_cursor}<button type="button" class="secondary" disabled={loading} on:click={() => search(true)}>Continue search</button>{/if}</div>
  </form>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if response}<div class="coverage" aria-live="polite"><strong>{response.coverage.combinations_completed} / {response.coverage.combinations_total} combinations searched</strong><span>{loading ? "Live results arriving" : response.complete ? "Catalog pass complete" : "Partial coverage"} · Bounded search · Best discovered results</span>{#if response.provider_statuses.some(s => !s.ok)}<span>{response.provider_statuses.filter(s => !s.ok).length} provider requests unavailable</span>{/if}</div>{#if response.errors.length}<details class="diagnostics"><summary>Search details</summary>{#each response.errors as detail}<p>{detail}</p>{/each}</details>{/if}{/if}
  <div class="results" aria-busy={loading}>
    {#if loading && !visibleRoutes.length}<div class="skeleton" aria-label="Loading route results"></div><div class="skeleton"></div>{:else if !visibleRoutes.length}<div class="empty"><h3>{searched ? routes.length ? "Quotes have expired" : "No matching routes discovered" : "Choose currencies or explore all"}</h3><p>{searched ? "Try another amount, continue into more combinations, or start a new search for fresh quotes." : "Select SBP for RUB, choose any supported method, or search across currencies."}</p></div>{/if}
    {#each visibleRoutes as route (route.route_id)}
      <article class="result">
        <div class="resultHead"><h3>{route.source_amount} {route.source_fiat} → {route.target_amount} {route.target_fiat}</h3><span class:confirmed={route.profit_confirmed} class="badge">{mode === "arbitrage" ? route.profit_confirmed ? "Estimated net profit" : "Profit unconfirmed" : route.missing_costs.length ? "Costs incomplete" : "Estimated exchange"}</span></div>
        <p>{route.source_settlement.method ?? route.source_settlement.network ?? "Method unverified"} · {route.source_settlement.venue ?? "Venue unverified"} → {route.target_settlement.method ?? route.target_settlement.network ?? "Method unverified"} · {route.target_settlement.venue ?? "Venue unverified"}</p>
        {#if route.route_path?.length}<p class="path">{route.route_path.join(" → ")}</p>{/if}
        <dl><div><dt>Starting value</dt><dd>{money(route.source_value_usd)}</dd></div><div><dt>{mode === "arbitrage" ? route.profit_confirmed ? "Estimated net profit in USD" : "Profit before missing costs in USD" : "Value received per $1 sent"}</dt><dd>{mode === "arbitrage" ? money(route.net_profit_usd ?? route.estimated_profit_before_missing_costs_usd) : route.value_received_per_input === null ? "Unavailable" : `$${route.value_received_per_input.toFixed(4)}`}</dd></div>{#if route.return_percent !== null}<div><dt>{route.profit_confirmed ? "Estimated net return" : "Return before missing costs"}</dt><dd>{route.return_percent.toFixed(4)}%</dd></div>{/if}</dl>
        {#if route.missing_costs.length}<p class="note">Missing costs: {route.missing_costs.map(cost => cost.replaceAll("_"," ")).join(", ")}</p>{/if}
        <details><summary>Route details and provider links</summary><p>Observed {new Date(route.observed_at).toLocaleTimeString()}. {route.quote_expires_at ? `Expires ${new Date(route.quote_expires_at).toLocaleTimeString()}.` : "Refresh this estimate before proceeding."}</p>{#each route.route_fees ?? [] as fee}<p>Quoted fee: {fee.amount} {fee.asset}</p>{/each}{#each route.warnings as warning}<p>{warning}</p>{/each}{#each links(route) as link,index}<a href={link} target="_blank" rel="noopener noreferrer">Open provider {index+1}</a>{/each}</details>
      </article>
    {/each}
  </div>
</section>

<style>
  .discovery { width: min(100%, 1120px); margin: 24px auto; color: var(--color-text, #181818); }
  form, .result, .empty, .coverage, .diagnostics { padding: 24px; border: 1px solid var(--color-border, #d4d4d4); border-radius: 16px; background: var(--color-paper, #fff); }
  .heading, .resultHead, .actions { display: flex; align-items: center; justify-content: space-between; gap: 16px; flex-wrap: wrap; }
  h2 { font-size: 24px; line-height: 32px; margin: 0; text-wrap: balance; } h3 { font-size: 18px; line-height: 28px; margin: 0; }
  p, label, select, input, dd, dt, summary { font-size: 14px; line-height: 20px; } p { text-wrap: pretty; margin: 8px 0; }
  .fields { display: grid; grid-template-columns: repeat(auto-fit,minmax(200px,1fr)); gap: 16px; margin: 24px 0; }
  label { display: grid; gap: 8px; font-weight: 600; } select, input:not([type=checkbox]) { box-sizing: border-box; width: 100%; padding: 12px; min-height: 48px; border: 1px solid var(--color-border, #ccc); border-radius: 8px; background: var(--color-paper, #fff); color: inherit; font-family: inherit; }
  .check { display: flex; align-items: center; gap: 8px; } .check input { width: 16px; height: 16px; accent-color: #272727; }
  button { cursor: pointer; padding: 8px 12px; min-height: 40px; border: 1px solid #272727; border-radius: 8px; background: #272727; color: #fff; font-family: inherit; font-size: 16px; line-height: 24px; font-weight: 600; transition: opacity 300ms cubic-bezier(0.22,1,0.36,1), transform 300ms cubic-bezier(0.22,1,0.36,1); }
  button.secondary { color: inherit; background: transparent; border-color: var(--color-border, #ccc); } button:hover { opacity: .8; } button:active { transform: scale(.98); } button:disabled, select:disabled { opacity: .5; cursor: default; } button:focus-visible, select:focus-visible, input:focus-visible, a:focus-visible, summary:focus-visible { outline: 2px solid #168451; outline-offset: 4px; }
  .actions { justify-content: flex-start; margin-top: 16px; } .note, dt, .coverage span { color: var(--color-muted, #666); } .coverage, .diagnostics { margin-top: 16px; display: grid; gap: 8px; } .coverage span { font-size: 14px; }
  .results { display: grid; gap: 16px; margin-top: 16px; } .result p { overflow-wrap: anywhere; } .badge { font-size: 12px; line-height: 16px; padding: 4px 8px; border: 1px solid var(--color-border, #ccc); border-radius: 8px; } .badge.confirmed { color: #168451; }
  dl { display: flex; gap: 24px; flex-wrap: wrap; margin: 16px 0; } dd { margin: 4px 0 0; font-size: 18px; line-height: 28px; font-weight: 600; } details { margin-top: 16px; } summary { cursor: pointer; } details a { display: inline-block; margin: 12px 16px 0 0; color: inherit; text-decoration: underline; } .error { padding: 16px; border: 1px solid #b42318; border-radius: 8px; color: #b42318; }
  .skeleton { height: 160px; border-radius: 16px; background: var(--color-border, #ddd); opacity: .4; } .empty p { max-width: 680px; }
  @media(max-width:600px) { form, .result, .empty, .coverage { padding: 16px; } .fields { grid-template-columns: 1fr; } }
  @media(prefers-reduced-motion:reduce) { button { transition: none; } }
</style>
