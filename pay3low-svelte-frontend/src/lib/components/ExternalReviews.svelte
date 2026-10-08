<script lang="ts">
  import { onMount } from "svelte";
  import { fetchProfileReviews, fetchProviderReviews, type ExternalReview, type ExternalReviewSnapshot } from "$lib/exchange";
  import { locale, t } from "$lib/i18n";

  export let provider: string | null = null;
  export let profileUrl: string | null = null;
  export let sourceUrl: string | null = null;
  export let sourceName = "source";
  let reviews: ExternalReview[] = [];
  let visible = 5;
  let updatedAt: string | null = null;
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
    updatedAt = snapshot.fetched_at;
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
    visible = 5;
    updatedAt = null;
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
  <div class="heading"><strong>{copy("Customer reviews")}</strong><span>{sourceName}{#if reviews.length} · {copy(reviews.length >= 100 ? "100+ reviews" : "{count} reviews", { count: reviews.length })}{/if}</span></div>
  {#if loading}
    <p class="status">{copy("Loading reviews")}</p>
  {:else if reviews.length}
    {#if updatedAt}<p class="updated">{copy("Fetched {date}", { date: new Date(updatedAt).toLocaleDateString(language) })}</p>{/if}
    <ul>{#each reviews.slice(0, visible) as review (review.id)}
      <li class="review">
        <span class="avatar" aria-hidden="true">
          {review.author.trim().slice(0, 1).toUpperCase() || "?"}
          {#if review.avatar_url}<img src={review.avatar_url} alt="" loading="lazy" decoding="async" referrerpolicy="no-referrer" on:error={(event) => { (event.currentTarget as HTMLImageElement).style.display = "none"; }} />{/if}
        </span>
        <div class="reviewContent">
          <div class="reviewMeta"><strong>{review.author}</strong>{#if review.created_at}<time datetime={review.created_at}>{new Date(review.created_at).toLocaleDateString(language)}</time>{/if}</div>
          {#if review.rating != null}<span class="stars" aria-label={profileUrl?.includes("binance.com") ? copy(review.rating === 5 ? "Positive feedback" : "Negative feedback") : `${review.rating} / 5`}>{"★".repeat(review.rating)}<span>{"☆".repeat(Math.max(0, 5 - review.rating))}</span></span>{/if}
          <p>{review.text}</p>
        </div>
      </li>
    {/each}</ul>
    {#if visible < reviews.length}<button type="button" class="more" on:click={() => visible += 10}>{copy("Show 10 more reviews")} ↓</button>{/if}
  {:else}
    <p class="status">{copy(failed && profileUrl?.startsWith("https://www.bybit.com/")
      ? "Bybit requires sign-in to access seller reviews"
      : failed ? "Reviews are temporarily unavailable" : "No written reviews found yet")}</p>
  {/if}
</section>

<style>
  .reviews { margin-top: 16px; padding: 14px; border: 1px solid var(--color-border); border-radius: 14px; background: var(--color-panel-soft); min-width: 0; }
  .heading { display: flex; align-items: baseline; justify-content: space-between; flex-wrap: wrap; gap: 8px; }
  .heading strong { font-size: 14px; font-weight: 800; letter-spacing: -.02em; }
  .heading span { color: var(--color-text-faint); font-size: 12px; }
  .updated, .status { margin: 5px 0 0; color: var(--color-text-faint); font-size: 12px; line-height: 1.5; }
  ul { list-style: none; margin: 12px 0 0; padding: 0; }
  .review { display: flex; gap: 11px; padding: 13px 0; border-top: 1px solid var(--color-border); min-width: 0; }
  .avatar { position: relative; z-index: 1; flex: 0 0 36px; width: 36px; height: 36px; display: grid; place-items: center; overflow: hidden; border: 1px solid #cfe0bf; border-radius: 50%; background: linear-gradient(145deg, #d9f28c, #8dbd2b); color: #30430b; font-size: 14px; font-weight: 850; }
  .avatar img { position: absolute; width: 100%; height: 100%; object-fit: cover; }
  .reviewContent { min-width: 0; flex: 1; }
  .reviewMeta { display: flex; flex-wrap: wrap; gap: 4px 9px; align-items: baseline; font-size: 12px; }
  .reviewMeta strong { font-weight: 800; }
  .reviewMeta time { color: var(--color-text-faint); font-size: 12px; }
  .stars { display: inline-block; margin-top: 3px; color: #819e27; letter-spacing: 1px; font-size: 14px; line-height: 1; }
  .stars span { color: var(--color-border-strong); }
  .reviewContent p { margin: 6px 0 0; overflow-wrap: anywhere; white-space: pre-wrap; color: var(--color-text-soft); font-size: 12px; line-height: 1.55; }
  .more { margin-top: 8px; border: 1px solid var(--color-border); border-radius: 9px; min-height: 44px; padding: 8px 12px; background: var(--color-accent-soft); color: var(--color-primary); cursor: pointer; font-size: 12px; font-weight: 800; }
  .more:hover { background: var(--color-accent); }
  :global(html[data-theme="dark"]) .reviews { background: #222; }
  :global(html[data-theme="dark"]) .stars { color: var(--color-accent); }
</style>
