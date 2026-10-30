<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { apiUrl } from "$lib/api";
  import { t, type Locale } from "$lib/i18n";
  import type { RouteCandidate } from "$lib/exchange";
  import { formatReferenceRate, routeReferenceRate, type ReferenceMarket, type ReferenceRatesResponse } from "$lib/reference-rates";

  export let from: string;
  export let to: string;
  export let market: ReferenceMarket = "noncash";
  export let crypto = false;
  export let ready = false;
  export let routes: RouteCandidate[] = [];
  export let now = Date.now();
  export let locale: Locale = "en";
  let mounted = false;
  let loading = false;
  let refreshing = false;
  let failed = false;
  let response: ReferenceRatesResponse | null = null;
  let controller: AbortController | null = null;
  let nextRefresh = 0;
  let generation = 0;
  let timer: ReturnType<typeof setInterval> | undefined;

  $: key = ready && from && to ? `${from}:${to}:${market}:${crypto}` : "";
  $: if (mounted) changePair(key);
  $: median = crypto ? routeReferenceRate(routes, from, to, now) : null;

  function changePair(pairKey: string) {
    controller?.abort();
    generation += 1;
    response = null;
    failed = false;
    loading = false;
    refreshing = false;
    nextRefresh = 0;
    if (pairKey && !crypto) void refresh();
  }

  async function refresh() {
    if (!key || crypto || document.visibilityState === "hidden" || Date.now() < nextRefresh || refreshing) return;
    const requestGeneration = generation;
    const query = new URLSearchParams({ from, to, market });
    const requestController = new AbortController();
    controller?.abort();
    controller = requestController;
    refreshing = true;
    loading = response === null;
    const timeout = setTimeout(() => requestController.abort(), 12_000);
    try {
      const result = await fetch(apiUrl(`/api/reference-rates?${query}`), { signal: requestController.signal, headers: { Accept: "application/json" } });
      if (!result.ok) throw new Error("Reference rate request failed");
      const data: ReferenceRatesResponse = await result.json();
      if (requestGeneration !== generation || requestController.signal.aborted) return;
      if (data.from !== from || data.to !== to || data.market !== market) throw new Error("Reference pair mismatch");
      response = data;
      failed = false;
      nextRefresh = Date.now() + (data.unavailable_sources.length || data.rates.some(rate => rate.stale) ? 60_000 : [from, to].includes("AMD") ? 300_000 : 3_600_000);
    } catch {
      if (requestGeneration !== generation) return;
      // Do not keep an old client-side snapshot after the server can no longer validate its age.
      response = null;
      failed = true;
      nextRefresh = Date.now() + 60_000;
    } finally {
      clearTimeout(timeout);
      if (requestGeneration === generation) { loading = false; refreshing = false; }
    }
  }

  onMount(() => {
    mounted = true;
    timer = setInterval(() => void refresh(), 60_000);
    document.addEventListener("visibilitychange", refresh);
  });
  onDestroy(() => {
    generation += 1;
    controller?.abort();
    if (timer) clearInterval(timer);
    if (typeof document !== "undefined") document.removeEventListener("visibilitychange", refresh);
  });
</script>

{#if ready && from && to}
  <div class="referenceRates" data-testid="reference-rates" aria-label={t("Reference rate", {}, locale)} aria-busy={loading}>
    {#if crypto}
      {#if median}
        <div class="rateRow"><span>{t("Reference rate", {}, locale)} · 1 {from} = {formatReferenceRate(median.rate, locale)} {to}</span><span class="details">{t("Route median", {}, locale)} · {t("{count} samples", { count: median.sample_count }, locale)}</span></div>
      {:else}<span class="details">{t("Rate available after route search", {}, locale)}</span>{/if}
    {:else if loading}
      <span class="skeleton" role="status" aria-label={t("Loading reference rate", {}, locale)}></span>
    {:else}
      {#each response?.rates ?? [] as rate}
        <div class="rateRow" data-testid="reference-rate-row">
          <span>{t("Reference rate", {}, locale)} · 1 {from} = {formatReferenceRate(rate.rate, locale)} {to}</span>
          <span class="details">
            {#each rate.sources as source, index}{#if index} + {/if}<a href={source.url} target="_blank" rel="noopener noreferrer">{source.name}</a><span> · {source.published_at.slice(0, 10)}</span>{/each}
            {#if rate.method === "median_bank_midpoint" || rate.sources.some(source => source.name === "Rate.am")}<span> · {t("Bank midpoint median", {}, locale)} · {t(market === "cash" ? "Cash" : "Non-cash", {}, locale)} · {t("{count} samples", { count: rate.sample_count }, locale)}</span>{/if}
            {#if rate.method === "cross_via_usd"}<span> · {t("Via USD", {}, locale)}</span>{/if}
            {#if rate.method === "identity"}<span>{t("Same currency", {}, locale)}</span>{/if}
            {#if rate.stale}<span class="stale"> · {t("Cached rate", {}, locale)}</span>{/if}
          </span>
        </div>
      {/each}
      {#each response?.unavailable_sources ?? [] as source}<span class="details"><a href={source.url} target="_blank" rel="noopener noreferrer">{source.name}</a> · {t("Reference rate unavailable", {}, locale)}</span>{/each}
      {#if failed || (response && !response.rates.length && !response.unavailable_sources.length)}<span class="details">{t("Reference rate unavailable", {}, locale)}</span>{/if}
    {/if}
  </div>
{/if}

<style>
  .referenceRates { display: flex; flex-direction: column; gap: 8px; margin-top: 12px; font-size: 14px; line-height: 20px; text-wrap: pretty; }
  .rateRow { display: flex; flex-direction: column; gap: 4px; }
  .details { color: #9b9b9b; font-size: 12px; line-height: 16px; }
  a { color: inherit; text-decoration: underline; text-underline-offset: 2px; transition: opacity 300ms cubic-bezier(0.22,1,0.36,1); }
  a:hover { opacity: 0.75; }
  a:active { opacity: 0.6; }
  a:focus-visible { outline: 2px solid currentColor; outline-offset: 2px; border-radius: 2px; }
  .stale { color: #d6b36a; }
  .skeleton { display: block; width: 75%; height: 16px; border-radius: 4px; background: #313131; }
</style>
