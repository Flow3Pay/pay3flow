<script lang="ts">
  import { onDestroy, onMount, tick } from "svelte";
  import type { VenueSearchStatus } from "$lib/exchange";
  import { locale, routeNoun, t } from "$lib/i18n";
  import { lockPageScroll } from "$lib/page-scroll-lock";
  import { navigatePicker } from "$lib/picker-keyboard";
  import PickerOptionCard from "./PickerOptionCard.svelte";

  type VenueOption = { id: string; label: string; iconUrl: string };

  export let venues: VenueOption[] = [];
  export let stats: Record<string, VenueSearchStatus> = {};
  export let selectedVenueId: string | null = null;
  export let onClose: () => void;
  export let onSelectVenue: (venueId: string) => void = () => {};

  let dialog: HTMLDivElement;
  let input: HTMLInputElement;
  let query = "";
  let detailVenue: VenueOption | null = null;
  let detail: HTMLDivElement;
  let detailAnchor: HTMLElement | null = null;
  let detailLeft = 0;
  let detailTop = 0;
  let unlockPage: (() => void) | undefined;

  const matches = (text: string, search: string) => search.normalize("NFKC").toLocaleLowerCase().replace(/[^\p{L}\p{N}]+/gu, " ").trim().split(" ").filter(Boolean).every(term => text.normalize("NFKC").toLocaleLowerCase().replace(/[^\p{L}\p{N}]+/gu, "").includes(term));
  const statusFor = (venue: VenueOption) => stats[venue.id.toLowerCase()];
  const routesFor = (venue: VenueOption) => statusFor(venue)?.routes_found ?? 0;
  const speed = (value: number | null) => value == null ? "—" : `${Math.round(value)} ms`;

  $: searchable = venues.filter(venue => matches(`${venue.id} ${venue.label}`, query));
  $: listed = [...searchable]
    .filter(venue => routesFor(venue) > 0 || venue.id.toLowerCase() === selectedVenueId)
    .sort((left, right) => routesFor(right) - routesFor(left) || left.label.localeCompare(right.label));

  function choose(venue: VenueOption) {
    onSelectVenue(venue.id.toLowerCase());
    onClose();
  }

  async function openDetail(venue: VenueOption, node: HTMLElement) {
    detailVenue = venue;
    detailAnchor = node;
    await tick();
    placeDetail();
  }

  function placeDetail() {
    if (!detail || !detailAnchor) return;
    const rect = detailAnchor.getBoundingClientRect();
    const margin = 12;
    const gap = 8;
    detailLeft = Math.max(margin, Math.min(rect.right - detail.offsetWidth, window.innerWidth - detail.offsetWidth - margin));
    detailTop = rect.bottom + gap + detail.offsetHeight > window.innerHeight - margin
      ? Math.max(margin, rect.top - detail.offsetHeight - gap)
      : rect.bottom + gap;
  }

  function closeDetail(restoreFocus = false) {
    const anchor = detailAnchor;
    detailVenue = null;
    detailAnchor = null;
    if (restoreFocus) void tick().then(() => anchor?.focus());
  }

  function windowPointerdown(event: PointerEvent) {
    if (!detailVenue) return;
    const target = event.target as Node;
    if (detail?.contains(target) || detailAnchor?.contains(target)) return;
    closeDetail();
  }

  function keydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      if (detailVenue) { event.preventDefault(); closeDetail(true); return; }
      onClose();
      return;
    }
    navigatePicker(event, dialog);
  }

  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return { destroy() { node.remove(); } };
  }

  onMount(async () => {
    unlockPage = lockPageScroll();
    await tick();
    input?.focus();
  });
  onDestroy(() => unlockPage?.());
</script>

<svelte:window on:keydown={keydown} on:pointerdown={windowPointerdown} on:resize={() => closeDetail()} />

<div class="backdrop" role="presentation" on:mousedown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
  <div class="dialog" bind:this={dialog} role="dialog" aria-modal="true" aria-label={t("Venues in these routes", {}, $locale)} tabindex="-1">
    <div class="searchRow">
      <label class="searchBox">
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" aria-hidden="true"><circle cx="11" cy="11" r="7" stroke="currentColor" stroke-width="1.7" /><path d="m20 20-4-4" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" /></svg>
        <input bind:this={input} bind:value={query} type="search" placeholder={t("Search venues", {}, $locale)} aria-label={t("Search venues", {}, $locale)} autocomplete="off" spellcheck="false" />
      </label>
      <button type="button" class="backButton" on:click={onClose} aria-label={t("Close venue list", {}, $locale)}><kbd>esc</kbd><svg width="17" height="17" viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="m7 7 10 10m0-10L7 17" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" /></svg></button>
    </div>
    <div class="titleBar"><h2 class="title">{t("Venues in these routes", {}, $locale)}</h2></div>
    <div class="body">
      <div class="methods" role="listbox" aria-label={t("Platforms", {}, $locale)} on:scroll={() => closeDetail()}>
        {#each listed as venue (venue.id)}
          {@const status = statusFor(venue)}
          {@const routes = routesFor(venue)}
          <div class="venueRow" data-testid="found-venue" title={`Found on ${venue.label}`}>
            {#snippet metricsAction()}
              <button
                type="button"
                class="rowMenuButton"
                aria-haspopup="dialog"
                aria-expanded={detailVenue?.id === venue.id}
                aria-label={t("Venue metrics", {}, $locale)}
                on:click|stopPropagation={(event) => {
                  const node = event.currentTarget as HTMLElement;
                  if (detailVenue?.id === venue.id) closeDetail(true);
                  else void openDetail(venue, node);
                }}
              ><span aria-hidden="true">⋮</span></button>
            {/snippet}
            <PickerOptionCard
              name={venue.label}
              iconUrl={venue.iconUrl}
              initials={venue.label.slice(0, 2).toUpperCase()}
              color="var(--color-panel)"
              category={t("Platforms", {}, $locale)}
              meta={status ? `${routes} ${routeNoun(routes, $locale)} · ${speed(status.average_response_ms)}` : `${routes} ${routeNoun(routes, $locale)}`}
              selected={selectedVenueId === venue.id.toLowerCase()}
              ariaLabel={t("Show {venue} routes", { venue: venue.label }, $locale)}
              trailing={metricsAction}
              onSelect={() => choose(venue)}
            />
          </div>
        {:else}
          <div class="empty">{t("No venues found", {}, $locale)}<span>{t("Try a different search.", {}, $locale)}</span></div>
        {/each}
      </div>
    </div>
  </div>

  {#if detailVenue}
    {@const status = statusFor(detailVenue)}
    <div class="venueDetail" bind:this={detail} role="dialog" aria-modal="false" aria-label={t("Venue metrics", {}, $locale)} style:left={`${detailLeft}px`} style:top={`${detailTop}px`} use:portal>
      <strong class="detailName">{detailVenue.label}</strong>
      <dl class="detailStats">
        <div><dt>{t("Routes found", {}, $locale)}</dt><dd>{status?.routes_found ?? 0}</dd></div>
        <div><dt>{t("Quotes received", {}, $locale)}</dt><dd>{status?.offers_found ?? 0}</dd></div>
        <div><dt>{t("Last response", {}, $locale)}</dt><dd>{speed(status?.last_response_ms ?? null)}</dd></div>
        <div><dt>{t("Average response", {}, $locale)}</dt><dd>{speed(status?.average_response_ms ?? null)}{#if (status?.response_samples ?? 0) > 1}<small>{t("{count} samples", { count: status?.response_samples ?? 0 }, $locale)}</small>{/if}</dd></div>
        <div><dt>{t("Cache hits", {}, $locale)}</dt><dd>{status?.cache_hits ?? 0}</dd></div>
      </dl>
      {#if status && !status.ok}
        <p class="detailNote">{t("Source failed", {}, $locale)}</p>
      {:else if (status?.routes_found ?? 0) === 0}
        <p class="detailNote">{(status?.offers_found ?? 0) > 0 ? t("Offers received · no matching route", {}, $locale) : t("No matching route", {}, $locale)}</p>
      {/if}
      <a class="detailLink" href={`/providers/${encodeURIComponent(detailVenue.id.toLowerCase())}`} on:click={onClose}>{t("Open platform profile", {}, $locale)}<span aria-hidden="true">↗</span></a>
    </div>
  {/if}
</div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 1100; display: grid; place-items: center; padding: 24px; background: #11172255; backdrop-filter: blur(5px); }
  .dialog { display: flex; flex-direction: column; width: min(100%, 480px); max-height: min(620px, 88dvh); overflow: hidden; border: 1px solid var(--color-border); border-radius: 12px; background: var(--color-paper); box-shadow: 0 24px 90px #11172224, 0 3px 12px #11172212; outline: none; }
  .searchRow { display: flex; align-items: center; gap: 6px; flex: 0 0 auto; min-height: 58px; padding: 6px 10px 6px 16px; border-bottom: 1px solid var(--color-border); }
  .searchBox { display: flex; align-items: center; gap: 10px; min-width: 0; flex: 1; min-height: 44px; color: var(--color-text-faint); }
  .searchBox svg { flex: 0 0 auto; }
  .searchBox input { width: 100%; min-width: 0; border: 0; padding: 0; outline: 0; background: transparent; color: var(--color-text); font: inherit; font-size: 14px; font-weight: 450; }
  .searchBox input::placeholder { color: var(--color-text-faint); opacity: 1; }
  .backButton { display: grid; place-items: center; flex: 0 0 44px; min-height: 44px; padding: 0; border-radius: 6px; color: var(--color-text-faint); }
  .backButton:hover { background: var(--color-panel-soft); }
  .backButton svg { display: none; }
  kbd { font: 10px var(--font-mono); border: 1px solid var(--color-border); padding: 3px 5px; border-radius: 4px; color: var(--color-text-faint); }
  .titleBar { display: flex; align-items: center; gap: 8px; padding: 10px 16px 2px; }
  .title { margin: 0; font-size: 11px; font-weight: 500; color: var(--color-text-soft); }
  .body { display: flex; flex-direction: column; min-height: 0; overflow: hidden; flex: 1; }
  .methods { min-height: 0; max-height: min(480px, calc(88dvh - 106px)); overflow-y: auto; overscroll-behavior: contain; padding: 4px 8px 12px; scrollbar-gutter: stable; }
  .venueRow { position: relative; }
  .rowMenuButton { display: grid; width: 28px; height: 32px; flex: 0 0 auto; place-items: center; border: 1px solid transparent; border-radius: 7px; background: transparent; color: var(--color-text-soft); font-size: 17px; line-height: 1; cursor: pointer; }
  .rowMenuButton:hover, .rowMenuButton[aria-expanded="true"] { border-color: var(--color-border-strong); background: var(--color-paper); color: var(--color-text); }
  .rowMenuButton:focus-visible { outline: var(--focus-ring-width) solid var(--color-focus); outline-offset: 1px; }
  .empty { display: grid; gap: 6px; padding: 28px 12px; color: var(--color-text-soft); text-align: center; font-size: 12px; }
  .empty span { color: var(--color-text-faint); }
  .venueDetail { position: fixed; z-index: 1200; display: grid; gap: 8px; width: min(280px, calc(100vw - 24px)); padding: 12px 14px; border: 1px solid var(--color-border-strong); border-radius: 10px; background: var(--color-paper); box-shadow: 0 18px 44px #11172233; }
  .detailName { font-size: 13px; font-weight: 650; }
  .detailStats { display: grid; gap: 4px; margin: 0; }
  .detailStats div { display: flex; align-items: baseline; justify-content: space-between; gap: 10px; }
  .detailStats dt { color: var(--color-text-faint); font-size: 11px; }
  .detailStats dd { margin: 0; color: var(--color-text); font-size: 11px; font-weight: 600; font-variant-numeric: tabular-nums; }
  .detailStats dd small { display: block; color: var(--color-text-faint); font-weight: 450; }
  .detailNote { margin: 0; color: var(--color-text-soft); font-size: 11px; }
  .detailLink { display: inline-flex; align-items: center; justify-content: space-between; gap: 8px; padding-top: 8px; border-top: 1px solid var(--color-border); color: var(--color-accent-text); font-size: 12px; font-weight: 600; }
  @media (max-width: 700px) {
    .backdrop { align-items: end; padding: 0; }
    .dialog { width: 100%; max-height: 86dvh; border-radius: 16px 16px 0 0; padding-bottom: env(safe-area-inset-bottom); }
    .searchRow { min-height: 56px; padding-left: 14px; }
    .searchBox input { font-size: 16px; }
    .methods { max-height: calc(86dvh - 106px); padding-bottom: 18px; }
    .backButton kbd { display: none; }
    .backButton svg { display: block; }
  }
</style>
