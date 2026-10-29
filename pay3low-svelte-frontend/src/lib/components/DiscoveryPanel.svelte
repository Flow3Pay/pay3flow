<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { apiUrl, wsUrl } from "$lib/api";
  import { mergeDiscoveryRoutes, routeExpired, type DiscoveryQuery, type DiscoveryResponse, type DiscoveryRoute } from "$lib/discovery";

  export let request: DiscoveryQuery;
  export let loading = false;
  export let catalogError = "";
  export let catalogLoading = false;
  let routes: DiscoveryRoute[] = [];
  let response: DiscoveryResponse | null = null;
  let searched = false, error = "";
  let socket: WebSocket | undefined, controller: AbortController | undefined;
  let sequence = 0, clock = Date.now(), lastQuery = "";
  $: mode = request.mode;
  $: visibleRoutes = routes.filter(route => !routeExpired(route, clock));
  $: if (JSON.stringify(request) !== lastQuery) { lastQuery = JSON.stringify(request); reset(); }

  function cancel() { sequence++; socket?.close(); socket = undefined; controller?.abort(); controller = undefined; loading = false; }
  export function reset() { cancel(); routes = []; response = null; error = ""; searched = false; }
  onMount(() => {
    const timer = setInterval(() => clock = Date.now(),1000);
    return () => clearInterval(timer);
  });
  onDestroy(cancel);
  function apply(next: DiscoveryResponse, continuation: boolean, previous: DiscoveryRoute[]) {
    response = next;
    routes = mergeDiscoveryRoutes(continuation ? previous : [],next.routes,mode);
  }
  export async function search(continuation = false) {
    const cursor = continuation ? response?.next_cursor : undefined;
    cancel(); const current = sequence;
    const amount = request.source_amount;
    if (!/^\d+(\.\d+)?$/.test(amount) || !Number.isFinite(Number(amount)) || Number(amount) <= 0) { error = "Enter an amount greater than zero."; return; }
    loading = true; searched = true; error = "";
    const previous = continuation ? routes : [];
    if (!continuation) { routes = []; response = null; }
    const payload = { ...request, ...(cursor ? { cursor } : {}) };
    let received = false;
    try {
      await new Promise<void>((resolve,reject) => {
        const ws = new WebSocket(wsUrl("/ws/p2p/discovery")); socket = ws;
        const timer = setTimeout(() => { ws.close(); reject(new Error("Search timed out. Try again.")); },65_000);
        const finish = (error?: Error) => { clearTimeout(timer); ws.close(); if (error) reject(error); else resolve(); };
        ws.onopen = () => ws.send(JSON.stringify(payload));
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
            for (const [key,value] of Object.entries(payload)) if (value !== undefined) url.searchParams.set(key,String(value));
            const reply = await fetch(url,{ signal:controller.signal });
            if (!reply.ok) throw new Error(await reply.text());
            if (current === sequence) apply(await reply.json(),continuation,previous);
          } finally { clearTimeout(timer); }
        } catch (fallback) { if (current === sequence) error = fallback instanceof Error ? fallback.message : "Search failed. Try again."; }
      } else error = cause instanceof Error ? cause.message : "Search interrupted. Try again.";
    } finally { if (current === sequence) loading = false; }
  }
  function money(value: number | null) { return value === null ? "Unavailable" : new Intl.NumberFormat("en-US",{style:"currency",currency:"USD",maximumFractionDigits:4}).format(value); }
  function links(route: DiscoveryRoute) {
    const candidates = [route.entry_offer?.source_url,...(route.cycle_legs ?? []).map(l => l.source_url),route.route_provider_url,route.exit_offer?.source_url];
    return [...new Set(candidates.filter((link): link is string => Boolean(link && /^https?:\/\//.test(link))))];
  }
</script>

<section class="discovery" aria-label="Found routes">
  <div class="actions">{#if loading}<button type="button" class="secondary" on:click={cancel}>Stop search</button>{/if}{#if response?.next_cursor}<button type="button" class="secondary" disabled={loading} on:click={() => search(true)}>Continue search</button>{/if}</div>
  {#if catalogLoading}<p role="status">Loading payment methods…</p>{/if}
  {#if catalogError}<p class="error" role="alert">Payment methods could not be loaded. Reload the page to choose bank methods.</p>{/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if response}<div class="coverage" aria-live="polite"><strong>{response.coverage.combinations_completed} / {response.coverage.combinations_total} combinations searched</strong><span>{loading ? "Live results arriving" : response.complete ? "Catalog pass complete" : "Partial coverage"} · Bounded search · Best discovered results</span>{#if response.provider_statuses.some(s => !s.ok)}<span>{response.provider_statuses.filter(s => !s.ok).length} provider requests unavailable</span>{/if}</div>{#if response.errors.length}<details class="diagnostics"><summary>Search details</summary>{#each response.errors as detail}<p>{detail}</p>{/each}</details>{/if}{/if}
  <div class="results" aria-busy={loading}>
    {#if loading && !visibleRoutes.length}<div class="skeleton" aria-label="Loading route results"></div><div class="skeleton"></div>{:else if !visibleRoutes.length}<div class="empty"><h3>{searched ? routes.length ? "Quotes have expired" : "No matching routes discovered" : "Choose currencies or explore all"}</h3><p>{searched ? "Try another amount, continue into more combinations, or start a new search for fresh quotes." : "Select currencies and payment methods in Exchange, then find routes."}</p></div>{/if}
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
  .discovery { width: 100%; margin: 0; color: var(--color-text, #181818); }
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
