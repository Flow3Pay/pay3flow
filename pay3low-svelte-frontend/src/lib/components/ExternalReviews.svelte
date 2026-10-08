<script lang="ts">
  import { onMount } from "svelte";
  import { fetchProfileReviews, fetchProviderReviews, type ExternalReview, type ExternalReviewSnapshot } from "$lib/exchange";
  import { locale, t } from "$lib/i18n";

  export let provider: string | null = null;
  export let profileUrl: string | null = null;
  export let sourceUrl: string | null = null;
  export let sourceName = "source";
  export let venueName = "";
  export let iconUrl = "";
  let reviews: ExternalReview[] = [];
  let expanded = new Set<string>();
  $: rated = reviews.filter(review => review.rating != null && review.rating >= 0 && review.rating <= 5);
  $: positive = rated.filter(review => review.rating! >= 4).length;
  $: average = rated.length ? rated.reduce((sum, review) => sum + review.rating!, 0) / rated.length : null;
  $: binaryFeedback = Boolean(profileUrl?.includes("binance.com"));
  function expandReview(id: string) { const next = new Set(expanded); if (next.has(id)) next.delete(id); else next.add(id); expanded = next; }
  function date(value: string) { const parsed = new Date(value); return Number.isNaN(parsed.getTime()) ? "" : parsed.toLocaleDateString(language, { day: "numeric", month: "short", year: "numeric" }); }
  function stars(value: number) { return Math.max(0, Math.min(5, Math.round(value))); }
  let visible = 10;
  let loading = true;
  let failed = false;
  let mounted = false;
  let loadedKey = "";
  let controller: AbortController | null = null;
  let requestId = 0;
  let refreshTimer: ReturnType<typeof setTimeout> | undefined;

  $: language = $locale;
  const copy = (key: string, params: Record<string, string | number> = {}) => t(key, params, language);

  $: reviewKey = `${provider ?? ""}|${profileUrl ?? ""}`;
  $: if (mounted && reviewKey !== loadedKey) {
    loadedKey = reviewKey;
    loadReviews();
  }

  function applySnapshot(snapshot: ExternalReviewSnapshot) {
    reviews = snapshot.reviews;
    sourceUrl = snapshot.source_url;
    if (sourceUrl.includes("forum.bits.media")) sourceName = "Bits.media";
    else if (sourceUrl.includes("otzovik.com")) sourceName = "Otzovik";
    else if (sourceUrl.includes("bestchange.com") || sourceUrl.includes("bestchange.biz")) sourceName = "BestChange";
    else if (sourceUrl.includes("papa-change.biz")) sourceName = "Papa Change";
    else if (sourceUrl.includes("id-pay.ru")) sourceName = "ID Pay";
    else if (sourceUrl.includes("yandex.com/maps")) sourceName = "Yandex Maps";
    else if (sourceUrl.includes("rustore.ru")) sourceName = "RuStore";
    else if (sourceUrl.includes("trustscores.org")) sourceName = "TrustScores";
  }

  function refreshBitget(url: string, currentRequest: number, attempt = 0) {
    if (attempt >= 3 || !controller || controller.signal.aborted) return;
    refreshTimer = setTimeout(() => {
      if (currentRequest !== requestId || !controller || controller.signal.aborted) return;
      fetchProfileReviews(url, controller.signal, true)
        .then((snapshot) => {
          if (currentRequest !== requestId) return;
          if (snapshot.reviews.length > reviews.length) applySnapshot(snapshot);
          if (snapshot.reviews.length) failed = false;
        })
        .catch(() => {})
        .finally(() => { if (currentRequest === requestId) refreshBitget(url, currentRequest, attempt + 1); });
    }, [5_000, 10_000, 20_000][attempt]);
  }

  function loadReviews() {
    controller?.abort();
    if (refreshTimer) clearTimeout(refreshTimer);
    controller = new AbortController();
    const currentRequest = ++requestId;
    reviews = [];
    visible = 10; expanded = new Set();
    loading = true;
    failed = false;
    const pending = provider ? fetchProviderReviews(provider, controller.signal)
      : profileUrl ? fetchProfileReviews(profileUrl, controller.signal)
      : null;
    if (pending) {
      pending.then((snapshot) => {
        if (currentRequest !== requestId) return;
        applySnapshot(snapshot);
      }).catch(() => { if (currentRequest === requestId) failed = true; }).finally(() => {
        if (currentRequest !== requestId) return;
        loading = false;
        if (profileUrl?.startsWith("https://www.bitget.com/")) refreshBitget(profileUrl, currentRequest);
      });
    } else {
      failed = true;
      loading = false;
    }
  }

  onMount(() => {
    mounted = true;
    return () => {
      controller?.abort();
      if (refreshTimer) clearTimeout(refreshTimer);
    };
  });
</script>

<section class="reviews" aria-label={copy("Customer reviews")}>
  <div class="reviewHero">
    <div class="heroOrbit" aria-hidden="true"></div>
    {#if iconUrl}<div class="heroBrand"><img src={iconUrl} alt={venueName} width="72" height="72" /><strong>{venueName}</strong></div>{/if}
    <div class="heroIntro"><span class="heroEyebrow">{copy("CUSTOMER VOICES")}</span><h2>{copy("Customer reviews")}</h2><p>{copy("Feedback from {source}", { source: sourceName })}</p>{#if sourceUrl}<a class="sourceLink" href={sourceUrl} target="_blank" rel="noopener noreferrer">{copy("View original source")} <span>↗</span></a>{/if}</div>
    {#if !loading && reviews.length}
      <div class="ratingSummary">
        <div class="score">{binaryFeedback && rated.length ? `${Math.round(positive / rated.length * 100)}%` : average != null ? average.toFixed(1) : String(reviews.length)}{#if !binaryFeedback && average != null}<span>/ 5</span>{/if}</div>
        <p>{copy(binaryFeedback && rated.length ? "Positive feedback in this sample" : average != null ? "Average of collected ratings" : "Written customer experiences")}</p>
        <span class="sampleSize">{copy("{count} collected reviews", { count: reviews.length })}</span>
      </div>
      {#if rated.length && !binaryFeedback}<div class="distribution" aria-label={copy("Rating distribution")}>{#each [5, 4, 3, 2, 1] as rating}{@const count = rated.filter(review => stars(review.rating!) === rating).length}<div><span>{rating} ★</span><div class="ratingTrack"><span style={`width:${count / rated.length * 100}%`}></span></div><small>{count}</small></div>{/each}</div>{/if}
    {:else}<span class="heroQuote" aria-hidden="true">“</span>{/if}
  </div>
  {#if loading}
    <p class="status" role="status">{copy("Loading reviews")}</p><div class="skeletonGrid" aria-hidden="true">{#each [1, 2, 3, 4] as card}<div class="skeletonCard"><span></span><i></i><i></i><i></i></div>{/each}</div>
  {:else if reviews.length}

    <ul class="reviewGrid">{#each reviews.slice(0, visible) as review, index (review.id)}
      <li class="review" style={`--order:${index};--avatar-hue:${[84, 170, 35, 220, 285][index % 5]}`}>
        <div class="reviewTop"><span class="avatar" aria-hidden="true">{review.author.trim().slice(0, 1).toUpperCase() || "?"}{#if review.avatar_url}<img src={review.avatar_url} alt="" loading="lazy" decoding="async" referrerpolicy="no-referrer" on:error={(event) => { (event.currentTarget as HTMLImageElement).style.display = "none"; }} />{/if}</span><div class="reviewMeta"><strong>{review.author}</strong>{#if review.created_at && date(review.created_at)}<time datetime={review.created_at}>{date(review.created_at)}</time>{:else}<span>{sourceName}</span>{/if}</div>{#if review.url}<a class="reviewOriginal" href={review.url} target="_blank" rel="noopener noreferrer" aria-label={copy("Open original review by {author}", { author: review.author })}>↗</a>{/if}</div>
        {#if review.rating != null && review.rating >= 0 && review.rating <= 5}{#if binaryFeedback}<span class="feedbackBadge" class:critical={review.rating < 4}>{copy(review.rating >= 4 ? "Positive feedback" : "Negative feedback")}</span>{:else}<span class="stars" aria-label={`${review.rating} / 5`}>{"★".repeat(stars(review.rating))}<span>{"☆".repeat(5 - stars(review.rating))}</span></span>{/if}{:else}<span class="unrated"><span aria-hidden="true">☆☆☆☆☆</span><small>{copy("Not rated")}</small></span>{/if}
        <p class="reviewText">{review.text.length > 360 && !expanded.has(review.id) ? `${review.text.slice(0, 360).trimEnd()}…` : review.text}</p>
        {#if review.text.length > 360}<button type="button" class="readMore" aria-expanded={expanded.has(review.id)} on:click={() => expandReview(review.id)}>{copy(expanded.has(review.id) ? "Show less" : "Read full review")} {expanded.has(review.id) ? "↑" : "↓"}</button>{/if}
      </li>
    {/each}</ul>
    {#if visible < reviews.length}<button type="button" class="more" on:click={() => visible += 10}>{copy("Show 10 more reviews")} <span>↓</span></button>{/if}
  {:else}
    <div class="emptyState"><span aria-hidden="true">“</span><h3>{copy(failed && profileUrl?.startsWith("https://www.bybit.com/") ? "Bybit requires sign-in to access seller reviews" : failed ? "Reviews are temporarily unavailable" : "No written reviews found yet")}</h3>{#if failed}<button type="button" on:click={loadReviews}>{copy("Try again")} ↻</button>{/if}</div>
  {/if}
</section>

<style>
  .reviews { min-width: 0; }
  .heroBrand { display: grid; justify-items: center; gap: 12px; flex: 0 0 80px; }
  .heroBrand img { width: 72px; height: 72px; object-fit: contain; border-radius: 18px; }
  .heroBrand strong { font-size: 11px; text-align: center; overflow-wrap: anywhere; }
  .unrated { display: flex; align-items: center; gap: 12px; margin-top: 18px; color: var(--color-text-faint); }
  .unrated > span { font-size: 20px; letter-spacing: 2px; }
  .unrated small { font-size: 10px; }

  .reviewHero { position: relative; isolation: isolate; overflow: hidden; display: flex; align-items: center; gap: 30px; padding: 34px; border-radius: 24px; background: #19251e; color: #f3f7ef; }
  .heroOrbit { position: absolute; z-index: -1; width: 340px; height: 340px; border-radius: 50%; border: 1px solid #b5f50014; right: -50px; top: -130px; box-shadow: 0 0 0 50px #b5f50004, 0 0 0 100px #b5f50003; }.heroIntro { flex: 1; min-width: 0; }.heroEyebrow { font: 9px var(--font-mono); letter-spacing: .14em; color: #b5f500; }.heroIntro h2 { margin-top: 14px; font-size: 24px; font-weight: 750; letter-spacing: -.035em; }.heroIntro p { margin-top: 9px; font-size: 12px; color: #bfcbbe; overflow-wrap: anywhere; }.sourceLink { display: inline-flex; align-items: center; gap: 15px; margin-top: 24px; min-height: 44px; border: 1px solid #ffffff30; border-radius: 9px; padding: 9px 12px; font-size: 11px; color: #f3f7ef; }.sourceLink:hover { background: #ffffff0d; }.sourceLink span { font-size: 18px; }
  .ratingSummary { flex: 0 1 200px; }.score { font-size: 66px; font-weight: 700; line-height: 1; letter-spacing: -.07em; color: #b5f500; }.score > span { margin-left: 7px; font-size: 18px; letter-spacing: -.02em; color: #bfcbbe; }.ratingSummary p { margin-top: 14px; font-size: 11px; line-height: 1.6; color: #c5cfc1; }.sampleSize { display: block; margin-top: 7px; color: #c5cfc1; font: 9px var(--font-mono); }.distribution { display: grid; gap: 11px; flex: 0 1 175px; }.distribution > div { display: flex; align-items: center; gap: 10px; }.distribution > div > span { white-space: nowrap; color: #c5cfc1; font: 10px var(--font-mono); }.ratingTrack { height: 5px; border-radius: 6px; background: #ffffff14; flex: 1; min-width: 50px; overflow: hidden; }.ratingTrack > span { display: block; height: 100%; background: #b5f500; border-radius: inherit; transition: width .5s; }.distribution small { min-width: 17px; text-align: right; font: 9px var(--font-mono); color: #c5cfc1; }.heroQuote { font: 170px Georgia, serif; color: #b5f500; height: 115px; }

  .reviewGrid { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; list-style: none; padding: 0; margin: 24px 0 0; align-items: start; }.review { position: relative; min-width: 0; padding: 23px; border: 1px solid var(--color-border); border-radius: 18px; background: var(--color-paper); transition: border-color .2s, box-shadow .2s; animation: cardIn .35s ease both; }.review:hover { border-color: var(--color-border-strong); box-shadow: 0 8px 25px #00000008; }.reviewTop { display: flex; align-items: center; gap: 12px; }.avatar { position: relative; flex: 0 0 42px; width: 42px; height: 42px; display: grid; place-items: center; overflow: hidden; border-radius: 50%; background: hsl(var(--avatar-hue) 45% 87%); color: hsl(var(--avatar-hue) 45% 24%); font-size: 16px; font-weight: 750; }.avatar img { position: absolute; width: 100%; height: 100%; object-fit: cover; }.reviewMeta { display: grid; gap: 6px; min-width: 0; }.reviewMeta strong { font-size: 12px; font-weight: 750; overflow-wrap: anywhere; }.reviewMeta time, .reviewMeta > span { font-size: 10px; color: var(--color-text-soft); }.reviewOriginal { display: grid; place-items: center; min-width: 44px; min-height: 44px; margin-left: auto; color: var(--color-text-soft); font-size: 20px; }.reviewOriginal:hover { color: var(--color-accent-text); }
  .stars { display: block; margin-top: 20px; color: var(--color-accent-text); letter-spacing: 3px; font-size: 17px; }.stars > span { color: var(--color-text-faint); }.feedbackBadge { display: inline-block; margin-top: 18px; padding: 6px 9px; border-radius: 6px; background: var(--color-accent-soft); color: var(--color-text); font-size: 10px; }.feedbackBadge.critical { background: var(--color-panel-soft); border: 1px solid var(--color-border); }.reviewText { margin-top: 15px; font-size: 13px; line-height: 1.9; white-space: pre-wrap; overflow-wrap: anywhere; color: var(--color-text); }.readMore { min-height: 44px; margin-top: 8px; color: var(--color-accent-text); font-size: 11px; }.more { display: flex; align-items: center; justify-content: center; gap: 25px; min-height: 48px; margin: 24px auto 0; padding: 12px 24px; border: 1px solid var(--color-border); border-radius: 12px; background: var(--color-paper); font-size: 12px; font-weight: 700; }.more:hover { background: var(--color-accent-soft); }.more span { font-size: 20px; }
  .emptyState { display: grid; justify-items: center; padding: 48px 20px; text-align: center; }.emptyState > span { font: 80px Georgia, serif; color: var(--color-accent-text); height: 65px; }.emptyState h3 { font-size: 14px; color: var(--color-text-soft); line-height: 1.8; }.emptyState button { min-height: 44px; padding: 10px 20px; margin-top: 20px; border: 1px solid var(--color-border); border-radius: 10px; font-size: 12px; }.status { padding: 24px 0; color: var(--color-text-soft); font-size: 12px; }.skeletonGrid { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; }.skeletonCard { padding: 24px; border-radius: 18px; border: 1px solid var(--color-border); background: var(--color-paper); }.skeletonCard span, .skeletonCard i { display: block; background: var(--color-border); border-radius: 5px; animation: pulse 1.5s ease-in-out infinite; }.skeletonCard span { width: 42px; height: 42px; border-radius: 50%; margin-bottom: 28px; }.skeletonCard i { height: 9px; margin-top: 15px; }.skeletonCard i:last-child { width: 65%; }
  @keyframes cardIn { from { opacity: 0; translate: 0 8px; } }@keyframes pulse { 50% { opacity: .4; } }
  @media (max-width: 1100px) { .reviewHero { flex-wrap: wrap; gap: 24px; padding: 27px; }.heroIntro { flex-basis: calc(100% - 110px); }.ratingSummary { flex: 1; }.distribution { flex: 1; max-width: 250px; } }
  @media (max-width: 760px) { .reviewHero { padding: 20px; gap: 16px; border-radius: 18px; }.heroBrand { flex-basis: 56px; }.heroBrand img { width: 56px; height: 56px; }.heroIntro { flex-basis: calc(100% - 72px); }.heroIntro h2 { font-size: 24px; }.score { font-size: 57px; }.ratingSummary { flex-basis: 120px; }.distribution { flex-basis: 110px; }.reviewGrid, .skeletonGrid { grid-template-columns: 1fr; }.review { padding: 21px; }.reviewText { font-size: 13px; }.heroQuote { display: none; } }
  @media (prefers-reduced-motion: reduce) { .review, .skeletonCard span, .skeletonCard i { animation: none; } }
</style>
