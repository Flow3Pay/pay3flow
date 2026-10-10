<script lang="ts">
  import { onMount } from 'svelte';
  import { replaceState } from '$app/navigation';
  import Header from './Header.svelte';
  import ProviderRankingChart from './ProviderRankingChart.svelte';
  import ProviderReviews from './ProviderReviews.svelte';
  import { fetchProviders } from '$lib/exchange';
  import { fallbackVenue, venuesFromCatalog, fetchProviderStatistics, percent, providerSwapHref, type ProfilePeriod, type ProviderStatistics } from '$lib/provider-profile';
  import { profileCopy } from '$lib/provider-profile-copy';
  import { locale } from '$lib/i18n';
  import { venueIcon, assetIcon } from '$lib/icons';
  import { SITE_URL } from '$lib/home-content';
  export let slug: string;
  let venue = fallbackVenue(slug);
  let statistics: ProviderStatistics | null = null;
  let period: ProfilePeriod = '30d';
  let tab: 'overview' | 'directions' | 'reviews' | 'about' = 'overview';
  let mounted = false, loading = true, failed = false, catalogFailed = false, unavailable = false;
  let saved = false, message = '', loadedKey = '';
  let statsController: AbortController | null = null;
  let catalogController: AbortController | null = null;
  let messageTimer: ReturnType<typeof setTimeout> | undefined;
  $: copy = profileCopy($locale);
  $: statsKey = `${slug}:${period}`;
  $: if (mounted && statsKey !== loadedKey) { loadedKey = statsKey; void loadStatistics(); }
  $: top10share = statistics ? percent(statistics.top10, statistics.searches) : null;
  $: top1share = statistics ? percent(statistics.top1, statistics.searches) : null;
  $: responseShare = statistics ? percent(statistics.successful_responses, statistics.response_samples) : null;
  $: sourceHost = host(venue.url);
  function host(url: string) { try { return new URL(url).hostname.replace(/^www\./, ''); } catch { return ''; } }
  function format(value: number | null | undefined, digits = 0) { return value == null ? '—' : value.toLocaleString($locale, { maximumFractionDigits: digits }); }
  function date(value: string, time = false) { return new Date(value).toLocaleString($locale, { day: 'numeric', month: 'short', ...(time ? { hour: '2-digit', minute: '2-digit' } : { year: 'numeric' }), timeZone: 'UTC' }); }
  async function loadStatistics() {
    statsController?.abort();
    const current = new AbortController(); statsController = current;
    loading = true; failed = false; statistics = null;
    try {
      const result = await fetchProviderStatistics(slug, period, AbortSignal.any([current.signal, AbortSignal.timeout(15000)]));
      if (!current.signal.aborted) statistics = result;
    } catch { if (!current.signal.aborted) failed = true; }
    finally { if (!current.signal.aborted) loading = false; }
  }
  async function loadCatalog() {
    const current = new AbortController(); catalogController = current;
    try {
      const catalog = venuesFromCatalog(await fetchProviders(AbortSignal.any([current.signal, AbortSignal.timeout(15000)])));
      const found = catalog.find(value => value.slug === slug);
      if (!current.signal.aborted) { if (found) venue = found; else unavailable = true; }
    } catch { if (!current.signal.aborted) catalogFailed = true; }
  }
  function choosePeriod(value: ProfilePeriod) {
    period = value;
    const url = new URL(location.href); url.searchParams.set('period', value); replaceState(url, {});
  }
  function notify(value: string) { message = value; clearTimeout(messageTimer); messageTimer = setTimeout(() => message = '', 3500); }
  function toggleSaved() {
    try {
      const stored: unknown = JSON.parse(localStorage.getItem('pay3flow.saved-platforms') ?? '[]');
      const list = Array.isArray(stored) ? stored.filter((value): value is string => typeof value === 'string') : [];
      const next = saved ? list.filter(value => value !== slug) : [...new Set([...list, slug])];
      localStorage.setItem('pay3flow.saved-platforms', JSON.stringify(next)); saved = !saved;
    } catch { notify(copy.saveFailed); }
  }
  async function share() {
    try { await navigator.clipboard.writeText(location.href); notify(copy.copied); }
    catch { notify(copy.copyFailed); }
  }
  onMount(() => {
    const value = new URL(location.href).searchParams.get('period');
    if (value === '7d' || value === '30d' || value === '90d') period = value;
    try { const stored: unknown = JSON.parse(localStorage.getItem('pay3flow.saved-platforms') ?? '[]'); saved = Array.isArray(stored) && stored.includes(slug); } catch {}
    mounted = true; void loadCatalog();
    return () => { statsController?.abort(); catalogController?.abort(); clearTimeout(messageTimer); };
  });
</script>

<svelte:head>
  <title>{venue.name} — {copy.profile} · Pay3Flow</title>
  <meta name="description" content={`${venue.name}: ${copy.subtitle}`} />
  <link rel="canonical" href={`${SITE_URL.replace(/\/$/, '')}/providers/${slug}`} />
</svelte:head>

<div class="appShell profileShell">
  <Header activePage="providers" />
  <main class="profilePage">
    <nav class="breadcrumbs" aria-label={`${copy.directory} / ${venue.name}`}><a href="/">Pay3Flow</a><span>/</span><a href="/providers">{copy.directory}</a><span>/</span><span>{venue.name}</span></nav>
    <section class="profileHero" aria-labelledby="venue-name">
      <div class="heroWatermark" aria-hidden="true">↗</div>
      <div class="heroIdentity">
        <div class="venueAvatar"><img src={venueIcon(slug)} alt={venue.name} width="78" height="78" on:error={event => { const image = event.currentTarget as HTMLImageElement; image.onerror = null; image.src = '/icons/venues/generic.svg'; }} /></div>
        <div class="identityCopy"><span class="eyebrow">PAY3FLOW / {copy.profile}</span><h1 id="venue-name">{venue.name}<span class="identityMark" title={copy.identity} aria-label={copy.identity}>✓</span></h1><div class="identityMeta">{#each venue.types as type}<span class="typeBadge" class:violet={type === 'spot'}>{copy[type]}</span>{/each}{#if venue.searchable !== null}<span class="availability" class:catalogOnly={!venue.searchable}><i></i>{venue.searchable ? copy.integrated : copy.catalogOnly}</span>{/if}</div></div>
      </div>
      <div class="heroBottom"><p>{copy.subtitle}</p><div class="heroActions">{#if venue.url}<a class="officialButton" href={venue.url} target="_blank" rel="noopener noreferrer">{sourceHost}<span>↗</span></a>{/if}<a class="exchangeButton" href={providerSwapHref(slug)}>{copy.findRoutes}<span>↗</span></a></div></div>
      <div class="quickActions"><button type="button" class:saved aria-pressed={saved} aria-label={saved ? copy.saved : copy.save} title={saved ? copy.saved : copy.save} on:click={toggleSaved}><svg viewBox="0 0 24 24" fill={saved ? 'currentColor' : 'none'} stroke="currentColor" stroke-width="1.5" aria-hidden="true"><path d="M7 4h10v16l-5-3-5 3z" /></svg></button><button type="button" aria-label={copy.shareProfile} title={copy.shareProfile} on:click={share}><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><path d="M12 16V3m-4 4 4-4 4 4M5 12v8h14v-8" /></svg></button></div>
    </section>
    {#if message}<div class="notice" role="status">{message}</div>{/if}
    {#if catalogFailed}<p class="catalogNotice">{copy.catalogFailed}</p>{/if}
    {#if unavailable}<p class="catalogNotice" role="status">{copy.unavailable}</p>{/if}

    <div class="profileToolbar"><nav class="profileTabs" aria-label={copy.profile}>{#each ['overview', 'directions', 'reviews', 'about'] as value}<button type="button" class:active={tab === value} aria-pressed={tab === value} on:click={() => tab = value as typeof tab}>{copy[value as typeof tab]}</button>{/each}</nav><div class="periodPicker" aria-label={copy.period}>{#each [['7d', copy.days7], ['30d', copy.days30], ['90d', copy.days90]] as [value, label]}<button type="button" class:active={period === value} aria-pressed={period === value} on:click={() => choosePeriod(value as ProfilePeriod)}>{label}</button>{/each}</div></div>

    {#if tab === 'overview'}
      <div class="metrics" aria-busy={loading}>
        <article class="metric highlight"><div class="metricLabel"><span>{copy.top10}</span><span class="metricSymbol" aria-hidden="true">↗</span></div><strong>{format(top10share, 1)}<small>%</small></strong><p>{statistics ? `${format(statistics.top10)} / ${format(statistics.searches)}` : copy.noValue}</p><small class="metricHint">{copy.top10hint}</small></article>
        <article class="metric"><div class="metricLabel"><span>{copy.top1}</span><span class="metricSymbol violet" aria-hidden="true">♜</span></div><strong>{format(statistics?.top1)}</strong><p>{top1share !== null ? `${format(top1share, 1)}%` : copy.noValue}</p><small class="metricHint">{copy.top1hint}</small></article>
        <article class="metric"><div class="metricLabel"><span>{copy.searches}</span><span class="metricSymbol" aria-hidden="true">⌕</span></div><strong>{format(statistics?.searches)}</strong><p>{copy.performanceSubtitle}</p><small class="metricHint">{copy.searchHint}</small></article>
        <article class="metric"><div class="metricLabel"><span>{copy.opens}</span><span class="metricSymbol" aria-hidden="true">↗</span></div><strong>{format(statistics?.site_opens)}</strong><p>Pay3Flow → {venue.name}</p><small class="metricHint">{copy.opensHint}</small></article>
      </div>
      <div class="analyticsGrid">
        <section class="panel rankingPanel" aria-labelledby="ranking-title"><div class="panelHeading"><div><span class="eyebrow">ANALYTICS</span><h2 id="ranking-title">{copy.ranking}</h2><p>{copy.rankingSubtitle}</p></div><span class="unit">%</span></div>
          {#if loading}<div class="chartState" role="status"><span class="stateIcon pulse" aria-hidden="true">↗</span><h3>{copy.loading}</h3></div>
          {:else if failed}<div class="chartState" role="status"><span class="stateIcon" aria-hidden="true">↻</span><h3>{copy.failedTitle}</h3><p>{copy.failedHint}</p><button type="button" on:click={loadStatistics}>{copy.retry} ↻</button></div>
          {:else if !statistics?.searches}<div class="chartState"><span class="stateIcon" aria-hidden="true">↗</span><h3>{copy.emptyTitle}</h3><p>{copy.emptyHint}</p></div>
          {:else}<ProviderRankingChart days={statistics.days} />{/if}
        </section>
        <aside class="performance"><div class="performanceIntro"><span class="eyebrow">PERFORMANCE</span><h2>{copy.performance}</h2><p>{copy.performanceSubtitle}</p></div><div class="rankRing" style:--share={`${top10share ?? 0}%`}><div><strong>{format(top10share, 1)}<small>%</small></strong><span>{copy.inTop10}</span></div></div><dl><div><dt>{copy.averageRank}</dt><dd>{statistics?.average_rank != null ? `#${format(statistics.average_rank, 1)}` : '—'}</dd></div><div><dt>{copy.response}</dt><dd>{responseShare === null ? '—' : `${format(responseShare, 1)}%`}</dd></div><div><dt>{copy.speed}</dt><dd>{statistics?.average_response_ms != null ? `${format(statistics.average_response_ms / 1000, 2)} ${copy.seconds}` : '—'}</dd></div></dl></aside>
      </div>
    {/if}

    {#if tab === 'overview' || tab === 'directions'}
      <section class="panel directionPanel" aria-labelledby="directions-title"><div class="panelHeading"><div><span class="eyebrow">EXCHANGE ROUTES</span><h2 id="directions-title">{copy.popular}</h2><p>{copy.popularSubtitle}</p></div><span class="countBadge">{statistics?.directions.length ?? '—'}</span></div>
        {#if loading}<p class="emptyDirections" role="status">{copy.loading}</p>{:else if failed}<div class="emptyDirections"><p>{copy.failedTitle}</p><button type="button" on:click={loadStatistics}>{copy.retry} ↻</button></div>{:else if statistics?.directions.length}
          <div class="tableScroll"><table><thead><tr><th>{copy.direction}</th><th>{copy.participation}</th><th>{copy.inTop10}</th><th>{copy.bestRank}</th><th><span class="srOnly">{copy.exchange}</span></th></tr></thead><tbody>{#each statistics.directions as direction}<tr><td><a class="currencyPair" href={providerSwapHref(slug, direction.source_currency, direction.target_currency)}><span class="pairIcons"><img src={assetIcon(direction.source_currency)} alt="" width="26" height="26" /><img src={assetIcon(direction.target_currency)} alt="" width="26" height="26" /></span><strong>{direction.source_currency}<span>→</span>{direction.target_currency}</strong></a></td><td class="numeric">{format(direction.searches)}</td><td><div class="shareCell"><span>{format(percent(direction.top10, direction.searches), 1)}%</span><i><b style:width={`${percent(direction.top10, direction.searches) ?? 0}%`}></b></i></div></td><td class="numeric">{direction.best_rank ? `#${direction.best_rank}` : '—'}</td><td><a class="directionLink" href={providerSwapHref(slug, direction.source_currency, direction.target_currency)} aria-label={`${copy.exchange} ${direction.source_currency} → ${direction.target_currency}`}>↗</a></td></tr>{/each}</tbody></table></div>
        {:else}<p class="emptyDirections">{copy.noDirections}</p>{/if}
      </section>
    {/if}
    <div class:hidden={tab !== 'overview' && tab !== 'reviews'}><ProviderReviews {slug} /></div>
    {#if tab === 'overview' || tab === 'about'}
      <section class="aboutGrid"><article class="panel aboutPanel"><span class="eyebrow">PLATFORM DETAILS</span><h2>{copy.about} {venue.name}</h2><p>{copy.aboutText}</p><dl><div><dt>{copy.availableTypes}</dt><dd>{#each venue.types as type}<span class="typeBadge">{copy[type]}</span>{:else}—{/each}</dd></div><div><dt>{copy.currencies}</dt><dd>{venue.currencies.length ? venue.currencies.join(' · ') : copy.allCurrencies}</dd></div><div><dt>{copy.paymentMethods}</dt><dd>{venue.banks.length ? venue.banks.join(' · ') : copy.noPaymentMethods}</dd></div>{#if statistics?.first_seen}<div><dt>{copy.since}</dt><dd>{date(statistics.first_seen)}</dd></div>{/if}</dl></article><article class="panel methodology"><span class="eyebrow">TRANSPARENCY</span><h2>{copy.methodology}</h2><p>{copy.methodologyText}</p><p>{copy.methodologyRank} {copy.timezone}</p><details><summary>{copy.fees} <span>+</span></summary><p>{venue.feeModel?.description ?? copy.feeHint}</p>{#if venue.feeModel?.docs_url}<a href={venue.feeModel.docs_url} target="_blank" rel="noopener noreferrer">{copy.feeDocs} ↗</a>{/if}</details></article></section>
    {/if}
    <div class="dataFooter"><span>PAY3FLOW / PLATFORM INSIGHTS</span>{#if statistics}<span>{copy.updated}: {date(statistics.updated_at, true)} UTC</span>{/if}</div>
  </main>
  <footer class="siteFooter"><span>Pay3Flow</span><a href="/providers">{copy.directory}</a><a href="/terms">{ $locale === 'ru' ? 'Условия использования' : 'Usage policy' }</a></footer>
</div>

<style>
  .profilePage { width: min(1180px, calc(100% - 2 * var(--page-gutter))); margin: 18px auto 0; }.breadcrumbs { display: flex; align-items: center; gap: 12px; color: var(--color-text-soft); font-size: 11px; margin-bottom: 22px; }.breadcrumbs a:hover { color: var(--color-accent-text); }.breadcrumbs > span:last-child { color: var(--color-text); }
  .profileHero { position: relative; padding: 36px; border: 1px solid var(--color-border); border-radius: 18px; background: linear-gradient(110deg, var(--color-paper) 55%, var(--color-accent-soft)); overflow: hidden; }.heroIdentity { position: relative; display: flex; align-items: center; gap: 23px; }.venueAvatar { width: 106px; height: 106px; flex-shrink: 0; display: grid; place-items: center; border: 1px solid var(--color-border); border-radius: 24px; background: var(--color-paper); }.venueAvatar img { width: 78px; height: 78px; object-fit: contain; border-radius: 14px; }.eyebrow { display: block; color: var(--color-text-soft); font-family: var(--font-mono); font-size: 10px; letter-spacing: .12em; text-transform: uppercase; }h1 { display: flex; align-items: center; gap: 14px; margin: 7px 0 12px; font-size: clamp(30px, 4vw, 46px); line-height: 1.1; letter-spacing: -1.9px; overflow-wrap: anywhere; }.identityMark { display: grid; width: 22px; height: 22px; flex-shrink: 0; place-items: center; border-radius: 50%; font-size: 12px; color: var(--color-accent-text); background: var(--color-accent-soft); letter-spacing: 0; }.identityMeta { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; }.typeBadge { display: inline-flex; padding: 5px 9px; background: var(--color-panel); border: 1px solid var(--color-border); border-radius: 5px; font-size: 10px; font-weight: 700; }.typeBadge.violet { color: var(--color-violet); border-color: transparent; background: var(--color-violet-soft); }.availability { display: flex; align-items: center; gap: 6px; margin-left: 8px; font-size: 10px; color: var(--color-text-soft); }.availability i { width: 5px; height: 5px; border-radius: 50%; background: var(--color-good); }.availability.catalogOnly i { background: var(--color-text-soft); }.heroBottom { position: relative; display: flex; align-items: flex-end; gap: 30px; justify-content: space-between; margin-top: 28px; }.heroBottom > p { max-width: 520px; color: var(--color-text-soft); font-size: 12px; line-height: 1.85; }.heroActions { display: flex; flex-shrink: 0; gap: 10px; }.heroActions a { display: inline-flex; min-height: 42px; align-items: center; gap: 24px; padding: 10px 16px; border: 1px solid var(--color-border-strong); border-radius: 7px; font-size: 11px; font-weight: 700; }.officialButton { background: var(--color-paper); }.heroActions .exchangeButton { color: #172110; border-color: #b5f500; background: #b5f500; }.heroActions a:hover { filter: brightness(.96); }.heroWatermark { position: absolute; right: 145px; top: -55px; color: var(--color-accent-text); opacity: .065; font-size: 300px; line-height: 1; font-family: var(--font-mono); pointer-events: none; }.quickActions { position: absolute; right: 30px; top: 30px; display: flex; gap: 6px; }.quickActions button { display: grid; place-items: center; width: 34px; height: 34px; border: 1px solid var(--color-border); border-radius: 7px; background: var(--color-paper); color: var(--color-text-soft); }.quickActions svg { width: 16px; height: 16px; }.quickActions button:hover, .quickActions .saved { color: var(--color-accent-text); border-color: var(--color-accent-text); }
  .profileToolbar { display: flex; justify-content: space-between; align-items: center; gap: 20px; margin: 25px 0; border-bottom: 1px solid var(--color-border); }.profileTabs { display: flex; gap: 24px; }.profileTabs button { position: relative; color: var(--color-text-soft); padding: 14px 1px 18px; font-size: 12px; font-weight: 700; }.profileTabs button.active { color: var(--color-text); }.profileTabs button.active::after { position: absolute; bottom: -1px; left: 0; right: 0; height: 3px; border-radius: 2px; background: var(--color-accent-text); content: ''; }.periodPicker { display: flex; gap: 3px; margin-bottom: 7px; padding: 3px; border: 1px solid var(--color-border); background: var(--color-panel); border-radius: 7px; }.periodPicker button { min-height: 29px; padding: 6px 12px; color: var(--color-text-soft); border-radius: 4px; font-family: var(--font-mono); font-size: 10px; }.periodPicker .active { color: var(--color-text); background: var(--color-paper); box-shadow: 0 1px 4px #0000000b; }
  .metrics { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 15px; margin-bottom: 20px; }.metric { min-height: 193px; padding: 23px; border: 1px solid var(--color-border); border-radius: 13px; background: var(--color-paper); }.metric.highlight { background: var(--color-accent-soft); border-color: color-mix(in srgb, var(--color-accent-text) 30%, var(--color-border)); }.metricLabel { display: flex; align-items: center; justify-content: space-between; gap: 5px; font-size: 11px; font-weight: 700; }.metricSymbol { display: grid; place-items: center; width: 26px; height: 26px; color: var(--color-text-soft); background: var(--color-panel); border-radius: 6px; font-size: 17px; }.metricSymbol.violet { color: var(--color-violet); background: var(--color-violet-soft); }.highlight .metricSymbol { background: #b5f500; color: #172110; }.metric > strong { display: block; margin-top: 18px; font-family: var(--font-mono); font-size: 36px; font-weight: 500; line-height: 1.2; letter-spacing: -1.5px; }.metric > strong small { margin-left: 3px; font-size: 20px; color: var(--color-text-soft); }.metric > p { min-height: 15px; font-family: var(--font-mono); font-size: 10px; color: var(--color-text-soft); margin-top: 7px; }.metricHint { display: block; margin-top: 17px; font-size: 10px; line-height: 1.65; color: var(--color-text-soft); }
  .analyticsGrid { display: grid; grid-template-columns: minmax(0, 1fr) 300px; gap: 20px; margin-bottom: 24px; }.panel { border: 1px solid var(--color-border); border-radius: 14px; background: var(--color-paper); padding: 28px; }.panelHeading { display: flex; justify-content: space-between; align-items: flex-start; gap: 15px; }h2 { margin: 8px 0; font-size: 21px; letter-spacing: -.65px; line-height: 1.4; }.panelHeading p, .performanceIntro p { color: var(--color-text-soft); font-size: 11px; line-height: 1.8; }.unit { font-family: var(--font-mono); font-size: 11px; border: 1px solid var(--color-border); border-radius: 5px; padding: 5px 10px; }.performance { padding: 28px 25px; background: #192019; color: #f6f7f3; border-radius: 14px; }.performance .eyebrow { color: #b4beb1; }.performance h2 { font-size: 18px; }.performanceIntro p { color: #b4beb1; font-size: 10px; }.rankRing { width: 156px; height: 156px; margin: 22px auto; display: grid; place-items: center; border-radius: 50%; background: conic-gradient(from -90deg, #b5f500 0 var(--share), #364034 var(--share) 100%); }.rankRing > div { width: 136px; height: 136px; border-radius: 50%; display: flex; flex-direction: column; align-items: center; justify-content: center; background: #192019; }.rankRing strong { color: #c1f83b; font-family: var(--font-mono); font-size: 31px; font-weight: 500; letter-spacing: -1.5px; }.rankRing strong small { font-size: 15px; margin-left: 2px; }.rankRing > div > span { color: #b4beb1; font-size: 10px; margin-top: 7px; }.performance dl > div { display: flex; justify-content: space-between; gap: 10px; padding: 13px 0; border-top: 1px solid #ffffff18; }.performance dt { color: #b4beb1; font-size: 10px; }.performance dd { font-family: var(--font-mono); font-size: 11px; white-space: nowrap; }
  .chartState { min-height: 295px; display: flex; flex-direction: column; align-items: center; justify-content: center; text-align: center; padding: 30px; }.stateIcon { display: grid; place-items: center; width: 54px; height: 54px; margin-bottom: 20px; border: 1px solid var(--color-border); border-radius: 13px; color: var(--color-accent-text); background: var(--color-panel); font-size: 24px; }.chartState h3 { font-size: 15px; }.chartState p { max-width: 370px; margin-top: 12px; color: var(--color-text-soft); font-size: 11px; line-height: 1.9; }.chartState button, .emptyDirections button { padding: 8px 16px; margin-top: 16px; border: 1px solid var(--color-border); border-radius: 6px; font-size: 12px; color: var(--color-accent-text); }.pulse { animation: pulse 1.5s infinite; }@keyframes pulse { 50% { opacity: .35; } }
  .directionPanel { margin-bottom: 36px; padding-bottom: 8px; }.countBadge { padding: 6px 10px; color: var(--color-text-soft); font-family: var(--font-mono); font-size: 11px; background: var(--color-panel); border-radius: 5px; }.tableScroll { overflow-x: auto; margin-top: 23px; }table { width: 100%; border-collapse: collapse; font-size: 11px; }th { text-align: left; color: var(--color-text-soft); font-size: 10px; font-weight: 500; padding: 0 12px 14px 0; }td { padding: 16px 16px 16px 0; border-top: 1px solid var(--color-border); }th:last-child, td:last-child { text-align: right; padding-right: 0; }.currencyPair { display: inline-flex; align-items: center; gap: 12px; white-space: nowrap; }.pairIcons { display: flex; padding-right: 4px; }.pairIcons img { object-fit: contain; border-radius: 50%; background: var(--color-paper); }.pairIcons img + img { margin-left: -7px; border: 2px solid var(--color-paper); }.currencyPair strong { font-size: 12px; }.currencyPair strong span { display: inline-block; margin: 0 9px; color: var(--color-text-soft); font-weight: 400; }.numeric { font-family: var(--font-mono); }.shareCell { display: flex; align-items: center; gap: 14px; font-family: var(--font-mono); font-size: 11px; }.shareCell span { min-width: 45px; }.shareCell i { display: block; width: 80px; height: 4px; background: var(--color-panel); overflow: hidden; border-radius: 4px; }.shareCell b { display: block; height: 100%; background: var(--color-accent-text); }.directionLink { display: inline-grid; width: 32px; height: 32px; place-items: center; color: var(--color-text-soft); border: 1px solid var(--color-border); border-radius: 6px; font-size: 17px; }.directionLink:hover { background: var(--color-accent); color: #172110; }.emptyDirections { text-align: center; padding: 40px 15px; color: var(--color-text-soft); font-size: 12px; }
  .aboutGrid { display: grid; grid-template-columns: 1fr 1fr; gap: 20px; margin-top: 35px; }.aboutGrid p { color: var(--color-text-soft); font-size: 11px; line-height: 1.95; margin-top: 13px; }.aboutPanel dl { margin-top: 20px; }.aboutPanel dl > div { display: grid; grid-template-columns: 150px 1fr; gap: 20px; padding: 13px 0; border-top: 1px solid var(--color-border); font-size: 11px; }.aboutPanel dt { color: var(--color-text-soft); }.aboutPanel dd { display: flex; flex-wrap: wrap; align-items: center; gap: 5px; overflow-wrap: anywhere; }details { margin-top: 23px; border-top: 1px solid var(--color-border); padding-top: 15px; }summary { display: flex; justify-content: space-between; cursor: pointer; font-size: 12px; font-weight: 700; list-style: none; }summary span { color: var(--color-text-soft); }details a { display: inline-block; margin-top: 13px; color: var(--color-accent-text); font-size: 11px; }
  .dataFooter { display: flex; justify-content: space-between; gap: 20px; margin-top: 27px; font-family: var(--font-mono); font-size: 9px; color: var(--color-text-soft); }.notice { padding: 12px 16px; margin-top: 12px; border: 1px solid var(--color-border); border-radius: 6px; background: var(--color-accent-soft); font-size: 12px; }.catalogNotice { margin-top: 12px; color: var(--color-text-soft); font-size: 11px; }.hidden { display: none; }.srOnly { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0, 0, 0, 0); }
  @media (max-width: 1000px) { .heroBottom { flex-wrap: wrap; gap: 20px; }.heroBottom > p { max-width: 580px; }.analyticsGrid { grid-template-columns: minmax(0, 1fr) 265px; }.metric { padding: 18px; }.metric > strong { font-size: 30px; }.panel { padding: 23px; } }
  @media (max-width: 760px) { .profileHero { padding: 26px; }.venueAvatar { width: 80px; height: 80px; border-radius: 18px; }.venueAvatar img { width: 57px; height: 57px; }.heroIdentity { gap: 16px; }.quickActions { right: 18px; top: 18px; }.identityCopy { min-width: 0; padding-right: 45px; }.identityCopy > .eyebrow { font-size: 8px; }h1 { letter-spacing: -1px; font-size: 31px; }.identityMeta { gap: 5px; }.availability { width: 100%; margin: 5px 0 0; }.metrics { grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; }.analyticsGrid { grid-template-columns: 1fr; }.performance { display: grid; grid-template-columns: 1fr 150px; gap: 5px 20px; }.performanceIntro { align-self: center; }.rankRing { grid-column: 2; grid-row: 1 / 3; margin: 0; width: 135px; height: 135px; align-self: center; }.rankRing > div { width: 117px; height: 117px; }.performance dl { margin-top: 10px; }.rankRing strong { font-size: 25px; }.aboutGrid { grid-template-columns: 1fr; }.profileToolbar { flex-wrap: wrap; gap: 12px; border-bottom: 0; }.profileTabs { width: 100%; gap: 26px; border-bottom: 1px solid var(--color-border); }.periodPicker { margin: 0 0 5px auto; }.metric { min-height: 190px; }.shareCell i { width: 40px; }.dataFooter { flex-wrap: wrap; gap: 10px; } }
  @media (max-width: 480px) { .profilePage { margin-top: 10px; }.breadcrumbs { margin-bottom: 16px; }.profileHero { padding: 20px; }.heroIdentity { align-items: flex-start; gap: 13px; }.venueAvatar { width: 65px; height: 65px; border-radius: 14px; }.venueAvatar img { width: 45px; height: 45px; }.identityCopy { padding-right: 0; padding-top: 4px; }h1 { font-size: 27px; gap: 8px; margin: 6px 0 10px; }.identityMark { width: 17px; height: 17px; font-size: 10px; }.quickActions { position: static; justify-content: flex-end; margin-top: 14px; }.heroBottom { margin-top: 20px; gap: 18px; }.heroActions { width: 100%; }.heroActions a { justify-content: space-between; flex: 1; padding: 10px 12px; gap: 10px; font-size: 10px; }.heroBottom > p { font-size: 11px; }.profileTabs { justify-content: space-between; gap: 8px; }.profileTabs button { font-size: 11px; }.metric { padding: 16px; min-height: 195px; }.metricLabel { font-size: 10px; }.metricSymbol { width: 22px; height: 22px; flex-shrink: 0; }.metric > strong { font-size: 29px; }.metric > p { font-size: 9px; }.metricHint { font-size: 9px; }.panel { padding: 20px; }h2 { font-size: 19px; }.performance { padding: 22px 18px; grid-template-columns: minmax(0, 1fr) 100px; gap: 0 12px; }.rankRing { width: 100px; height: 100px; }.rankRing > div { width: 85px; height: 85px; }.rankRing strong { font-size: 20px; }.rankRing strong small { font-size: 11px; }.performance h2 { font-size: 15px; }.performance dl > div { flex-wrap: wrap; gap: 5px; }.chartState { min-height: 240px; padding: 20px 5px; }.directionPanel { padding-bottom: 5px; }.tableScroll { margin-right: -10px; }.tableScroll table { min-width: 500px; }.aboutPanel dl > div { grid-template-columns: 110px 1fr; gap: 12px; font-size: 10px; } }
</style>
