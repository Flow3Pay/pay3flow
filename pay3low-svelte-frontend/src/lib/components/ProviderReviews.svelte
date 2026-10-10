<script lang="ts">
  import { onMount } from 'svelte';
  import { fetchProviderReviews, type ExternalReview, type ExternalReviewSnapshot } from '$lib/exchange';
  import { locale } from '$lib/i18n';
  import { profileCopy } from '$lib/provider-profile-copy';
  export let slug: string;
  let snapshot: ExternalReviewSnapshot | null = null;
  let loading = true, failed = false, filter: 'all' | 'positive' | 'critical' = 'all', visible = 6;
  let expanded = new Set<string>();
  let controller: AbortController | null = null;
  $: copy = profileCopy($locale);
  $: reviews = snapshot?.reviews ?? [];
  $: rated = reviews.filter(review => review.rating !== null && review.rating >= 0 && review.rating <= 5);
  $: average = rated.length ? rated.reduce((sum, review) => sum + review.rating!, 0) / rated.length : null;
  $: filtered = reviews.filter(review => filter === 'all' || (review.rating !== null && review.rating >= 0 && review.rating <= 5 && (filter === 'positive' ? review.rating >= 4 : review.rating < 4)));
  function chooseFilter(value: typeof filter) { filter = value; visible = 6; }
  function date(value: string) { const parsed = new Date(value); return Number.isNaN(parsed.getTime()) ? '' : parsed.toLocaleDateString($locale, { day: 'numeric', month: 'short', year: 'numeric', timeZone: 'UTC' }); }
  function stars(review: ExternalReview) { return review.rating !== null && review.rating >= 0 && review.rating <= 5 ? Math.round(review.rating) : null; }
  function toggle(id: string) { const next = new Set(expanded); next.has(id) ? next.delete(id) : next.add(id); expanded = next; }
  function sourceName(url: string) { try { return new URL(url).hostname.replace(/^www\./, ''); } catch { return copy.original; } }
  async function load() {
    controller?.abort();
    const current = new AbortController(); controller = current;
    loading = true; failed = false;
    try { snapshot = await fetchProviderReviews(slug, AbortSignal.any([current.signal, AbortSignal.timeout(20000)])); }
    catch { if (!current.signal.aborted) failed = true; }
    finally { if (!current.signal.aborted) loading = false; }
  }
  onMount(() => { void load(); return () => controller?.abort(); });
</script>

<section class="reviewSection" id="profile-reviews" aria-labelledby="profile-reviews-title">
  <div class="sectionHeading"><div><span class="eyebrow">COMMUNITY</span><h2 id="profile-reviews-title">{copy.reviews}</h2><p>{copy.reviewsSubtitle}</p></div>{#if snapshot?.source_url}<a class="source" href={snapshot.source_url} target="_blank" rel="noopener noreferrer">{sourceName(snapshot.source_url)} ↗</a>{/if}</div>
  {#if loading}<p class="status" role="status">{copy.reviewLoading}</p>
  {:else if failed}<div class="empty"><span aria-hidden="true">“</span><h3>{copy.reviewsFailed}</h3><button type="button" on:click={load}>{copy.reviewRetry} ↻</button></div>
  {:else if !reviews.length}<div class="empty"><span aria-hidden="true">“</span><h3>{copy.noReviews}</h3><p>{copy.noReviewsHint}</p></div>
  {:else}
    <div class="reviewSummary">
      <div class="score"><strong>{average === null ? '—' : average.toFixed(1)}</strong><span>{copy.rating}<small>{reviews.length} {copy.collected}</small></span></div>
      <div class="distribution" aria-label={copy.rating}>
        {#each [5, 4, 3, 2, 1] as rating}
          {@const count = rated.filter(review => Math.round(review.rating!) === rating).length}
          <div><span>{rating} ★</span><i><b style:width={`${rated.length ? count / rated.length * 100 : 0}%`}></b></i><small>{count}</small></div>
        {/each}
      </div>
      <div class="filters" aria-label={copy.reviews}>
        {#each [['all', copy.reviewAll], ['positive', copy.positive], ['critical', copy.critical]] as [value, label]}
          <button type="button" class:active={filter === value} aria-pressed={filter === value} on:click={() => chooseFilter(value as typeof filter)}>{label}</button>
        {/each}
      </div>
    </div>
    <div class="reviewGrid">
      {#each filtered.slice(0, visible) as review (review.id)}
        {@const rating = stars(review)}
        <article class="review">
          <div class="reviewHead"><span class="avatar">{review.author.trim().slice(0, 1).toUpperCase() || '?'}{#if review.avatar_url}<img src={review.avatar_url} alt="" loading="lazy" referrerpolicy="no-referrer" on:error={event => (event.currentTarget as HTMLImageElement).hidden = true} />{/if}</span><div><strong>{review.author}</strong>{#if review.created_at && date(review.created_at)}<time datetime={review.created_at}>{date(review.created_at)}</time>{:else}<small>{copy.original}</small>{/if}</div>{#if review.url}<a href={review.url} target="_blank" rel="noopener noreferrer" aria-label={`${copy.original}: ${review.author}`}>↗</a>{/if}</div>
          <div class="stars" aria-label={rating !== null ? `${review.rating} / 5` : copy.unrated}>{#if rating !== null}{'★'.repeat(rating)}<span>{'★'.repeat(5 - rating)}</span>{:else}<small>{copy.unrated}</small>{/if}</div>
          <p>{review.text.length > 280 && !expanded.has(review.id) ? `${review.text.slice(0, 280)}…` : review.text}</p>
          {#if review.text.length > 280}<button class="readMore" type="button" aria-expanded={expanded.has(review.id)} on:click={() => toggle(review.id)}>{expanded.has(review.id) ? copy.showLess : copy.readMore}</button>{/if}
        </article>
      {:else}<p class="status">{copy.noFilterReviews}</p>{/each}
    </div>
    {#if visible < filtered.length}<button class="more" type="button" on:click={() => visible += 6}>{copy.more} ↓</button>{/if}
  {/if}
</section>

<style>
  .reviewSection { padding-top: 8px; }.sectionHeading { display: flex; align-items: center; justify-content: space-between; gap: 20px; margin-bottom: 22px; }.eyebrow { color: var(--color-text-soft); font-family: var(--font-mono); font-size: 10px; letter-spacing: .12em; }h2 { margin: 8px 0; font-size: 24px; letter-spacing: -.7px; }.sectionHeading p { font-size: 12px; color: var(--color-text-soft); line-height: 1.8; }.source { flex-shrink: 0; font-family: var(--font-mono); font-size: 11px; }.source:hover { color: var(--color-accent-text); }
  .reviewSummary { display: flex; align-items: center; gap: 30px; border: 1px solid var(--color-border); background: var(--color-paper); border-radius: 14px; padding: 24px; margin-bottom: 16px; }.score { display: flex; align-items: center; gap: 18px; }.score strong { font-family: var(--font-mono); font-size: 56px; font-weight: 500; letter-spacing: -3px; }.score span { font-size: 12px; font-weight: 700; }.score small { display: block; margin-top: 8px; color: var(--color-text-soft); font-size: 10px; font-weight: 400; }
  .distribution { width: 160px; margin-left: 10px; }.distribution div { display: flex; align-items: center; gap: 8px; margin: 5px 0; font-size: 10px; }.distribution span { width: 24px; flex-shrink: 0; }.distribution i { height: 4px; flex: 1; overflow: hidden; border-radius: 3px; background: var(--color-panel); }.distribution b { display: block; height: 100%; background: var(--color-accent-text); }.distribution small { width: 22px; text-align: right; color: var(--color-text-soft); font-family: var(--font-mono); }
  .filters { display: flex; flex-wrap: wrap; gap: 5px; margin-left: auto; }.filters button { border: 1px solid var(--color-border); padding: 7px 12px; border-radius: 6px; font-size: 11px; color: var(--color-text-soft); }.filters button.active { color: var(--color-text); background: var(--color-accent-soft); border-color: var(--color-accent-text); }
  .reviewGrid { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 14px; }.review { padding: 23px; background: var(--color-paper); border: 1px solid var(--color-border); border-radius: 12px; }.reviewHead { display: flex; align-items: center; gap: 10px; }.avatar { position: relative; display: grid; width: 34px; height: 34px; flex: 0 0 auto; place-items: center; border-radius: 50%; overflow: hidden; color: var(--color-accent-text); background: var(--color-accent-soft); font-size: 12px; font-weight: 700; }.avatar img { position: absolute; width: 100%; height: 100%; object-fit: cover; }.reviewHead strong { display: block; font-size: 12px; overflow-wrap: anywhere; }.reviewHead time, .reviewHead small { display: block; margin-top: 4px; font-size: 10px; color: var(--color-text-soft); }.reviewHead a { margin-left: auto; align-self: flex-start; color: var(--color-text-soft); }.stars { color: var(--color-accent-text); font-size: 14px; letter-spacing: 2px; margin: 19px 0 12px; }.stars span { color: var(--color-border-strong); }.stars small { color: var(--color-text-soft); font-size: 10px; letter-spacing: 0; }.review p { font-size: 12px; line-height: 1.9; white-space: pre-line; overflow-wrap: anywhere; }.readMore { margin-top: 12px; color: var(--color-accent-text); font-size: 11px; font-weight: 650; }.more { display: block; margin: 22px auto 0; padding: 10px 18px; border: 1px solid var(--color-border); border-radius: 6px; font-size: 12px; }
  .status { padding: 30px 0; font-size: 13px; color: var(--color-text-soft); }.empty { padding: 32px; background: var(--color-paper); border: 1px solid var(--color-border); border-radius: 14px; text-align: center; }.empty > span { color: var(--color-violet); font-family: Georgia, serif; font-size: 56px; line-height: 1; }.empty h3 { font-size: 15px; margin: 8px 0; }.empty p { font-size: 12px; color: var(--color-text-soft); }.empty button { margin-top: 14px; color: var(--color-accent-text); font-size: 12px; }
  @media (max-width: 980px) { .reviewGrid { grid-template-columns: repeat(2, minmax(0, 1fr)); }.reviewSummary { flex-wrap: wrap; }.filters { width: 100%; margin-left: 0; } }
  @media (max-width: 600px) { .reviewGrid { grid-template-columns: 1fr; }.reviewSummary { padding: 18px; gap: 14px; }.score strong { font-size: 42px; }.score { gap: 10px; }.distribution { width: 105px; margin-left: auto; }.sectionHeading { flex-wrap: wrap; gap: 8px; }h2 { font-size: 21px; }.review { padding: 20px; } }
</style>
