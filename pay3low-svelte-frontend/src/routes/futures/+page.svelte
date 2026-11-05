<!--
Futures page draft paused by product decision. The route redirects to SWAP from
the server load in `+page.server.ts`. Preserve this implementation for a future
approved release.
<script lang="ts">
  import { onDestroy } from "svelte";
  import Header from "$lib/components/Header.svelte";
  import { searchFutures, type FuturesSearchResponse } from "$lib/futures-search";
  import { venueIcon } from "$lib/icons";
  import { locale } from "$lib/i18n";
  import { SITE_URL } from "$lib/home-content";

  const venues = ["binance", "bybit", "okx", "bitget"];
  let asset = "BTC";
  let notionalUsdt = "1000";
  let selectedVenues = [...venues];
  let response: FuturesSearchResponse | null = null;
  let error = "";
  let loading = false;
  let controller: AbortController | null = null;
  $: russian = $locale === "ru";
  $: armenian = $locale === "hy";
  $: copy = russian
    ? {
        eyebrow: "PAY3FLOW / FUTURES",
        title: "Арбитраж фьючерсов",
        intro: "Сравниваем котировки USDT-perpetual на Binance, Bybit, OKX и Bitget. Выберите актив и номинал в USDT, чтобы найти положительный межбиржевой спред.",
        asset: "Базовый актив",
        notional: "Номинал позиции (USDT)",
        venues: "Биржи",
        search: "Найти спреды",
        searching: "Сравниваем котировки…",
        contract: "Контракт",
        long: "Открыть long",
        short: "Открыть short",
        spread: "Валовый спред",
        profit: "Оценка валовой прибыли",
        empty: "Положительных межбиржевых спредов сейчас не найдено.",
        details: "Оценка рассчитана по лучшим bid/ask; в реальных сделках цены и доступный объем могут отличаться.",
      }
    : armenian
      ? {
          eyebrow: "PAY3FLOW / FUTURES",
          title: "Ֆյուչերսների արբիտրաժ",
          intro: "Համեմատում ենք Binance, Bybit, OKX և Bitget բորսաների USDT-perpetual գները։ Ընտրեք ակտիվն ու USDT անվանական ծավալը։",
          asset: "Հիմնական ակտիվ",
          notional: "Պայմանագրի ծավալ (USDT)",
          venues: "Բորսաներ",
          search: "Գտնել սպրեդներ",
          searching: "Համեմատում ենք գները…",
          contract: "Պայմանագիր",
          long: "Բացել long",
          short: "Բացել short",
          spread: "Համախառն սպրեդ",
          profit: "Համախառն շահույթի գնահատում",
          empty: "Դրական միջբորսայական սպրեդներ չեն գտնվել։",
          details: "Գնահատումը հիմնված է լավագույն bid/ask գների վրա. իրական գները և հասանելի ծավալը կարող են տարբերվել։",
        }
      : {
          eyebrow: "PAY3FLOW / FUTURES",
          title: "Perpetual futures arbitrage",
          intro: "Compare USDT perpetual quotes on Binance, Bybit, OKX and Bitget. Choose an asset and USDT notional to find positive cross-exchange spreads.",
          asset: "Underlying asset",
          notional: "Position notional (USDT)",
          venues: "Exchanges",
          search: "Find spreads",
          searching: "Comparing quotes…",
          contract: "Contract",
          long: "Open long",
          short: "Open short",
          spread: "Gross spread",
          profit: "Estimated gross profit",
          empty: "No positive cross-exchange spreads found right now.",
          details: "Estimate uses best bid/ask quotes; actual fill prices and available order-book depth may differ.",
        };

  function toggleVenue(venue: string) {
    selectedVenues = selectedVenues.includes(venue)
      ? selectedVenues.filter((selected) => selected !== venue)
      : [...selectedVenues, venue];
  }

  async function submitSearch(event: SubmitEvent) {
    event.preventDefault();
    const normalizedAsset = asset.trim().toUpperCase();
    const parsedNotional = Number(notionalUsdt);
    if (!/^[A-Z0-9]{2,12}$/.test(normalizedAsset) || normalizedAsset === "USDT") {
      error = russian ? "Введите тикер актива, например BTC или ETH." : "Enter an asset ticker such as BTC or ETH.";
      return;
    }
    if (!Number.isFinite(parsedNotional) || parsedNotional < 0.01 || parsedNotional > 100_000_000) {
      error = russian ? "Номинал должен быть от 0.01 до 100000000 USDT." : "Notional must be between 0.01 and 100000000 USDT.";
      return;
    }
    if (!selectedVenues.length) {
      error = russian ? "Выберите хотя бы одну биржу." : "Select at least one exchange.";
      return;
    }

    controller?.abort();
    controller = new AbortController();
    loading = true;
    error = "";
    response = null;
    try {
      response = await searchFutures(
        normalizedAsset,
        parsedNotional,
        selectedVenues.length === venues.length ? [] : selectedVenues,
        controller.signal,
      );
    } catch (cause) {
      if (!controller.signal.aborted) {
        error = cause instanceof Error ? cause.message : "The futures search failed.";
      }
    } finally {
      if (!controller.signal.aborted) loading = false;
    }
  }

  function venueName(venue: string) {
    return venue[0].toUpperCase() + venue.slice(1);
  }

  function money(value: number) {
    return new Intl.NumberFormat($locale === "ru" ? "ru-RU" : $locale === "hy" ? "hy-AM" : "en-US", {
      minimumFractionDigits: 2,
      maximumFractionDigits: 4,
    }).format(value);
  }

  onDestroy(() => controller?.abort());
</script>

<svelte:head>
  <title>{copy.title} — Pay3Flow</title>
  <meta name="description" content={copy.intro} />
  <link rel="canonical" href={`${SITE_URL.replace(/\/$/, "")}/futures`} />
</svelte:head>

<div class="appShell">
  <Header activePage="futures" />
  <main class="futuresPage">
    <span class="eyebrow">{copy.eyebrow}</span>
    <h1>{copy.title}<span>↗</span></h1>
    <p class="lead">{copy.intro}</p>

    <form class="searchPanel" on:submit={submitSearch}>
      <label>
        <span>{copy.asset}</span>
        <input bind:value={asset} maxlength="12" autocomplete="off" spellcheck="false" />
      </label>
      <label>
        <span>{copy.notional}</span>
        <input bind:value={notionalUsdt} type="number" min="0.01" max="100000000" step="any" />
      </label>
      <fieldset>
        <legend>{copy.venues}</legend>
        <div class="venues">
          {#each venues as venue}
            <button type="button" class:selected={selectedVenues.includes(venue)} aria-pressed={selectedVenues.includes(venue)} on:click={() => toggleVenue(venue)}>
              <img src={venueIcon(venue)} alt="" width="22" height="22" />
              {venueName(venue)}
            </button>
          {/each}
        </div>
      </fieldset>
      <button class="submit" type="submit" disabled={loading}>{loading ? copy.searching : copy.search}</button>
    </form>

    {#if error}<p class="error" role="alert">{error}</p>{/if}

    {#if response}
      <section class="results" aria-live="polite">
        <div class="resultsHeading">
          <div><span class="eyebrow">{response.asset} / USDT</span><h2>{response.opportunities.length} {russian ? "возможностей" : armenian ? "հնարավորություն" : "opportunities"}</h2></div>
          <span class="asOf">{new Date(response.searched_at).toLocaleTimeString()}</span>
        </div>
        {#if response.opportunities.length}
          <div class="tableWrap">
            <table>
              <thead><tr><th>{copy.contract}</th><th>{copy.long}</th><th>{copy.short}</th><th>{copy.spread}</th><th>{copy.profit}</th></tr></thead>
              <tbody>
                {#each response.opportunities as opportunity}
                  <tr>
                    <td><strong>{opportunity.contract}</strong><small>USDT perpetual</small></td>
                    <td>
                      {#if opportunity.long_url}<a href={opportunity.long_url} target="_blank" rel="noreferrer noopener">{venueName(opportunity.long_venue)} ↗</a>{:else}<span>{venueName(opportunity.long_venue)}</span>{/if}
                      <small>Ask {money(opportunity.long_ask)}</small>
                    </td>
                    <td>
                      {#if opportunity.short_url}<a href={opportunity.short_url} target="_blank" rel="noreferrer noopener">{venueName(opportunity.short_venue)} ↗</a>{:else}<span>{venueName(opportunity.short_venue)}</span>{/if}
                      <small>Bid {money(opportunity.short_bid)}</small>
                    </td>
                    <td class="spread">+{(opportunity.spread_bps / 100).toFixed(3)}%</td>
                    <td class="profit">+{money(opportunity.estimated_gross_profit_usdt)} USDT</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {:else}
          <p class="empty">{copy.empty}</p>
        {/if}
        <p class="details">{copy.details}</p>
        <ul class="warnings">{#each response.warnings as warning}<li>{warning}</li>{/each}</ul>
        {#if response.sources.some((source) => !source.ok)}
          <p class="sourceErrors">{response.sources.filter((source) => !source.ok).map((source) => `${venueName(source.source)}: ${source.error}`).join(" · ")}</p>
        {/if}
      </section>
    {/if}
  </main>
  <footer class="siteFooter"><span>Pay3Flow</span><a href="/#/swap">SWAP ↗</a></footer>
</div>

<style>
  .futuresPage { width: min(1180px, calc(100% - 2 * var(--page-gutter))); min-height: calc(100vh - 180px); margin: 52px auto; }
  .eyebrow { color: var(--color-text-soft); font-family: var(--font-mono); font-size: 10px; letter-spacing: .12em; }
  h1 { display: flex; align-items: center; gap: 16px; margin: 16px 0; font-size: clamp(30px, 4vw, 44px); letter-spacing: -1.5px; }
  h1 span { color: var(--color-accent-text); }
  .lead { max-width: 680px; color: var(--color-text-soft); font-size: 13px; line-height: 1.9; }
  .searchPanel { display: grid; grid-template-columns: minmax(130px, .7fr) minmax(180px, 1fr) minmax(300px, 1.8fr) auto; align-items: end; gap: 14px; margin: 34px 0; padding: 20px; border: 1px solid var(--color-border); border-radius: 13px; background: var(--color-paper); }
  label { display: grid; gap: 8px; color: var(--color-text-soft); font-size: 11px; }
  input { width: 100%; min-width: 0; height: 42px; padding: 0 12px; border: 1px solid var(--color-border); border-radius: 7px; background: var(--color-panel); color: var(--color-text); font: inherit; }
  fieldset { min-width: 0; margin: 0; padding: 0; border: 0; }
  legend { margin-bottom: 8px; color: var(--color-text-soft); font-size: 11px; }
  .venues { display: flex; flex-wrap: wrap; gap: 6px; }
  .venues button { display: inline-flex; align-items: center; gap: 7px; min-height: 40px; padding: 6px 9px; border: 1px solid var(--color-border); border-radius: 7px; background: var(--color-panel); color: var(--color-text-soft); font-size: 10px; }
  .venues button.selected { border-color: var(--color-accent-text); color: var(--color-text); background: var(--color-accent-soft); }
  .venues img { border-radius: 50%; }
  .submit { min-height: 42px; padding: 0 17px; border-radius: 7px; background: var(--color-accent); color: #172110; font-size: 11px; font-weight: 800; white-space: nowrap; }
  .submit:disabled { opacity: .6; cursor: wait; }
  .error, .sourceErrors { color: var(--color-danger, #c44); font-size: 12px; line-height: 1.6; }
  .results { margin-top: 40px; }
  .resultsHeading { display: flex; justify-content: space-between; align-items: end; gap: 16px; margin-bottom: 15px; }
  h2 { margin: 7px 0 0; font-size: 21px; letter-spacing: -.5px; }
  .asOf { color: var(--color-text-soft); font-family: var(--font-mono); font-size: 10px; }
  .tableWrap { overflow-x: auto; border: 1px solid var(--color-border); border-radius: 11px; background: var(--color-paper); }
  table { width: 100%; border-collapse: collapse; text-align: left; }
  th, td { padding: 14px 16px; border-bottom: 1px solid var(--color-border); white-space: nowrap; }
  th { color: var(--color-text-soft); font-size: 10px; font-weight: 600; }
  td { font-size: 12px; }
  td small { display: block; margin-top: 5px; color: var(--color-text-soft); font-family: var(--font-mono); font-size: 10px; }
  td a { color: var(--color-text); font-weight: 700; }
  td a:hover { color: var(--color-accent-text); }
  .spread, .profit { color: var(--color-accent-text); font-family: var(--font-mono); font-weight: 700; }
  .empty { padding: 22px; border: 1px solid var(--color-border); border-radius: 10px; color: var(--color-text-soft); font-size: 12px; }
  .details, .warnings { color: var(--color-text-soft); font-size: 11px; line-height: 1.7; }
  .details { margin: 15px 0 6px; }
  .warnings { padding-left: 18px; }
  .sourceErrors { margin-top: 15px; }
  @media (max-width: 940px) { .searchPanel { grid-template-columns: 1fr 1fr; }.searchPanel fieldset { grid-column: 1 / -1; }.submit { grid-column: 1 / -1; } }
  @media (max-width: 560px) { .futuresPage { margin-top: 30px; }.searchPanel { grid-template-columns: 1fr; padding: 15px; }.searchPanel fieldset, .submit { grid-column: auto; }.venues { display: grid; grid-template-columns: 1fr 1fr; }.resultsHeading { align-items: start; }.tableWrap { margin-right: calc(-1 * var(--page-gutter)); border-right: 0; border-radius: 10px 0 0 10px; }th, td { padding: 12px; font-size: 10px; } }
</style>
-->
