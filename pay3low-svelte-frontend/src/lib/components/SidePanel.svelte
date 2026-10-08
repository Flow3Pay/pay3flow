<script lang="ts">
  import { flip } from "svelte/animate";
  import { quintOut } from "svelte/easing";
  import { fly } from "svelte/transition";
  import type { RouteCandidate, ServiceVote, VenueSearchStatus } from "$lib/exchange";
  import { formatRouteCount, locale, t, type Locale } from "$lib/i18n";
  import { assetIcon, dislikeIcon, likeIcon, networkIcon, venueIcon, warningIcon } from "$lib/icons";
  import { fiatFlagUrl } from "$lib/currency-flags";
  import { routeScrollColor } from "$lib/route-scroll-color";

  export let routes: RouteCandidate[];
  export let routesFound = 0;
  export let sourceCurrency = "";
  export let targetCurrency = "";
  export let selectedRouteId: string | null;
  export let onSelect: (route: RouteCandidate) => void;
  export let onOpenInstructions: (route: RouteCandidate) => void;
  export let onVote: (route: RouteCandidate, vote: ServiceVote) => void;
  export let searching = false;
  export let renderingRoutes = false;
  export let searchingVenues: { id: string; label: string; iconUrl: string }[] = [];
  export let foundVenues: { id: string; label: string; iconUrl: string }[] = [];
  export let venueStats: Record<string, VenueSearchStatus> = {};
  export let venueNames: Record<string, string> = {};
  export let networkNames: Record<string, string> = {};
  export let searched = false;
  export let hasAmount = false;
  export let showBelarusP2pWarning = false;
  export let onOpenBelarusP2pWarning: () => void = () => {};
  export let onOpenSearchActivity: () => void = () => {};
  let selectedVenueId: string | null = null;
  let lastPair = "";

  const ASSET_NAMES: Record<string, string> = { BTC: "Bitcoin", ETH: "Ether", USDC: "USD Coin", USDT: "Tether" };
  const FIAT_CURRENCIES = new Set(["AMD", "BYN", "KZT", "RUB", "UAH", "USD"]);
  type Step = { currency: string; network?: string; networkLabel?: string; provider?: string; iconUrl?: string; paymentMethod?: string; paymentMethodIcon?: string };

  const venueName = (value?: string) => value ? venueNames[value.toLowerCase()] ?? value : "Searching";
  const readableNetwork = (value?: string) => value ? networkNames[value.toLowerCase()] ?? value.replace(/-/g, " ").replace(/\b\w/g, (letter) => letter.toUpperCase()) : "";
  const compactNetwork = (value?: string) => {
    if (!value) return "";
    const label = readableNetwork(value);
    const standard = label.match(/\(([^)]+)\)/)?.[1];
    if (standard) return standard;
    if (/c-chain/i.test(label)) return "C-Chain";
    if (/polygon/i.test(label)) return "Polygon";
    if (/arbitrum/i.test(label)) return "Arbitrum";
    if (/optimism/i.test(label)) return "Optimism";
    if (/cosmos/i.test(label)) return "Cosmos";
    return label;
  };
  const assetLabel = (currency?: string) => !currency ? "—" : currency.toUpperCase() === "USDT" ? currency : ASSET_NAMES[currency.toUpperCase()] ? `${currency} ${ASSET_NAMES[currency.toUpperCase()]}` : currency;
  const money = (minor?: number, currency?: string, exact?: string) => {
    if (exact && ["BTC", "ETH", "USDC", "USDT", "SOL", "TRX", "TON", "XRP", "ADA", "AVAX", "DOT", "LINK", "LTC", "BCH", "BNB", "DOGE", "MATIC", "NEAR", "SUI", "APT", "ATOM", "UNI", "DAI", "FDUSD"].includes(currency?.toUpperCase() ?? "")) {
      return `${Number(exact).toLocaleString("en-US", { maximumFractionDigits: 8, useGrouping: false })} ${currency ?? ""}`;
    }
    return minor == null ? "—" : `${(minor / 100).toLocaleString("en-US", { maximumFractionDigits: 2, useGrouping: false })} ${currency ?? ""}`;
  };
  const spreadLabel = (bps: number, language: Locale) => Math.abs(bps / 100) < 0.005
    ? t("Same output", {}, language)
    : t("{percent}% less", { percent: Math.abs(bps / 100).toFixed(2) }, language);
  const profitLabel = (route: RouteCandidate) => {
    const profit = route.profitability;
    if (!profit) return "";
    const minor = profit.status === "confirmed" ? profit.net_profit_minor : profit.gross_profit_minor;
    const bps = profit.status === "confirmed" ? profit.profit_bps : profit.gross_profit_bps;
    const sign = minor > 0 ? "+" : "";
    const decimals = route.profitability_decimals ?? 2;
    return `${sign}${(minor / 10 ** decimals).toLocaleString("en-US", { maximumFractionDigits: decimals, useGrouping: false })} ${route.source_currency} (${sign}${(bps / 100).toFixed(2)}%)`;
  };
  const compact = (value: number) => Intl.NumberFormat("en", { notation: "compact", maximumFractionDigits: 1 }).format(value);
  const routeCountLabel = (count: number, language: Locale) => formatRouteCount(count, language);
  const reduceMotion = () => typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const routeFlipDuration = (distance: number) => reduceMotion() || routes.length > 12 ? 0 : Math.min(680, 260 + distance * 0.65);
  const routeEnterDuration = () => reduceMotion() || routes.length > 12 ? 0 : 380;
  $: pendingVenues = searchingVenues.filter((venue) => !foundVenues.some((found) => found.id === venue.id));
  $: routeVenues = foundVenues.filter((venue) => (venueStats[venue.id.toLowerCase()]?.routes_found ?? 0) > 0);
  $: visiblePendingVenues = pendingVenues.slice(0, 5);
  $: hasHiddenPendingVenues = pendingVenues.length > visiblePendingVenues.length;
  $: circularSearch = sourceCurrency.toUpperCase() === targetCurrency.toUpperCase();
  $: if (`${sourceCurrency}:${targetCurrency}` !== lastPair) {
    lastPair = `${sourceCurrency}:${targetCurrency}`;
    selectedVenueId = null;
  }
  $: visibleRoutes = selectedVenueId
    ? routes.filter((route) => route.legs.some((leg) => leg.provider.toLowerCase() === selectedVenueId) || route.route_provider?.toLowerCase() === selectedVenueId || route.market_path?.venue.toLowerCase() === selectedVenueId)
    : routes;
  $: selectedVenueName = foundVenues.find((venue) => venue.id.toLowerCase() === selectedVenueId)?.label ?? selectedVenueId;

  function pathStepProvider(route: RouteCandidate, index: number, lastIndex: number) {
    if (index === 0) return undefined;
    if (route.cycle_legs?.length) return route.cycle_legs.find((leg) => leg.to_asset === route.route_path?.[index])?.provider;
    if (route.route_kind === "fiat_to_fiat" || route.route_kind === "fiat_cycle" || route.route_kind === "crypto_cycle") {
      if (index === 1 && route.entry_offer_snapshot) return route.entry_offer_snapshot.source;
      if (index === lastIndex && route.exit_offer_snapshot) return route.exit_offer_snapshot.source;
      return route.route_provider ?? undefined;
    }
    if (route.route_kind === "fiat_to_crypto") {
      if (index === 1 && route.entry_offer_snapshot) return route.entry_offer_snapshot.source;
      return route.route_provider ?? undefined;
    }
    if (route.route_kind === "crypto_to_fiat") {
      if (index === lastIndex && route.exit_offer_snapshot) return route.exit_offer_snapshot.source;
      return route.route_provider ?? undefined;
    }
    return route.route_provider ?? undefined;
  }

  function currencySteps(route: RouteCandidate): Step[] {
    const entry = route.legs.find((leg) => leg.kind === "entry");
    const exit = route.legs.find((leg) => leg.kind === "exit");
    const source = route.source_currency ?? "—";
    const target = route.target_currency ?? "—";
    if (route.route_path?.length) {
      return route.route_path.map((qualified, index) => {
        const [currency, network] = qualified.split("@", 2);
        return { currency, network, networkLabel: compactNetwork(network), provider: pathStepProvider(route, index, route.route_path!.length - 1) };
      });
    }
    if (!entry && exit) return [{ currency: source, network: route.source_network, provider: exit.provider, iconUrl: route.source_method_icon_url }, { currency: target, iconUrl: route.target_method_icon_url }];
    if (entry && !exit) return [{ currency: source, network: route.source_network, iconUrl: route.source_method_icon_url }, { currency: target, network: route.target_network, provider: entry.provider, iconUrl: route.target_method_icon_url }];
    if (route.bridge_currency) return [{ currency: source, network: route.source_network, provider: entry?.provider, iconUrl: route.source_method_icon_url }, { currency: route.bridge_currency }, { currency: target, network: route.target_network, provider: exit?.provider, iconUrl: route.target_method_icon_url }];
    return [{ currency: source, iconUrl: route.source_method_icon_url }, { currency: route.entry_asset ?? "—", network: route.entry_network !== "internal" ? route.entry_network : undefined, provider: entry?.provider }, { currency: target, provider: exit?.provider, iconUrl: route.target_method_icon_url }];
  }

  function workflowSteps(route: RouteCandidate): Step[] {
    const steps = currencySteps(route);
    return steps.map((step, index) => {
      if (!FIAT_CURRENCIES.has(step.currency.toUpperCase())) return step;
      if (index === 0) return { ...step, paymentMethod: route.source_payment_method, paymentMethodIcon: route.source_method_icon_url };
      if (index === steps.length - 1) return { ...step, paymentMethod: route.target_payment_method, paymentMethodIcon: route.target_method_icon_url };
      return step;
    });
  }

  const workflowLabel = (route: RouteCandidate) => workflowSteps(route).map((step) => `${assetLabel(step.currency)}${step.paymentMethod ? ` · ${step.paymentMethod}` : ""}${step.network ? ` · ${step.networkLabel ?? compactNetwork(step.network)}` : ""}${step.provider ? ` (${venueName(step.provider)})` : ""}`).join(" → ");

  function cardClick(event: MouseEvent, route: RouteCandidate) {
    onSelect(route);
    const target = event.target as HTMLElement;
    if (target.closest(".routeAmount") || target.closest(".workflow")) onOpenInstructions(route);
  }
  function fallbackVenueIcon(event: Event, provider: string) {
    const image = event.currentTarget as HTMLImageElement;
    image.onerror = null;
    image.src = venueIcon(provider);
  }
  function fallbackFoundVenueIcon(event: Event) {
    const image = event.currentTarget as HTMLImageElement;
    image.onerror = null;
    image.hidden = true;
    const fallback = image.nextElementSibling as HTMLElement | null;
    if (fallback) fallback.hidden = false;
  }
</script>

<aside class="side active" aria-label="Found routes" aria-busy={searching} id="routes">
  <div class="panel">
    <div class="panelTop">
      <div class="panelHeading">
        {#if hasAmount}
          <strong>Send {sourceCurrency} → {targetCurrency}</strong>
        {:else}
          <strong>{t("Awaiting your intent", {}, $locale)}</strong>
        {/if}
        {#if hasAmount}
          <span class="resultSummary">
            {#if hasAmount}<small aria-live="polite">{routeCountLabel(routesFound, $locale)}{searching ? " · searching…" : ""}</small>{/if}
            {#if routeVenues.length}
              <span class="foundVenues" aria-label={`Venues used in found routes: ${routeVenues.map((venue) => venue.label).join(", ")}`}>
                {#each routeVenues as venue, index (venue.id)}
                  {@const status = venueStats[venue.id.toLowerCase()]}
                  {@const hasRoute = (status?.routes_found ?? 0) > 0}
                  <div class:foundVenueActive={selectedVenueId === venue.id.toLowerCase()} class="foundVenue" data-testid="found-venue" title={`Found on ${venue.label}`} style:animation-delay={`${index * 70}ms`}>
                    <button type="button" class="foundVenueButton" aria-label={`Show ${venue.label} routes`} aria-pressed={selectedVenueId === venue.id.toLowerCase()} disabled={!hasRoute} on:click={() => selectedVenueId = selectedVenueId === venue.id.toLowerCase() ? null : venue.id.toLowerCase()}>
                      <img src={venue.iconUrl || venueIcon(venue.id)} alt="" width="18" height="18" decoding="async" on:error={fallbackFoundVenueIcon} />
                      <span class="foundVenueFallback" aria-hidden="true" hidden>{venue.label.slice(0, 1).toUpperCase()}</span>
                    </button>
                    <div class="foundVenuePopover" role="tooltip">
                      <div class="foundVenuePopoverTitle">
                        <img src={venue.iconUrl || venueIcon(venue.id)} alt="" width="20" height="20" decoding="async" on:error={(event) => fallbackVenueIcon(event, venue.id)} />
                        <strong>{venue.label}</strong>
                      </div>
                      {#if !hasRoute || status?.ok === false}<span class:venueOk={status?.ok} class:venueError={status && !status.ok}>{status?.ok === false ? (status.offers_found > 0 ? "Some quotes unavailable" : "Response error") : (status?.offers_found ?? 0) > 0 ? "Offers received · no matching route" : "No matching route"}</span>{/if}
                      <dl>
                        <div><dt>Routes found</dt><dd>{status?.routes_found ?? 0}</dd></div>
                        <div><dt>Quotes / markets received</dt><dd>{status?.offers_found ?? 0}</dd></div>
                        {#if status?.last_response_ms != null}<div><dt>Last response</dt><dd>{status.last_response_ms} ms</dd></div>{/if}
                        {#if status?.average_response_ms != null}<div><dt>Average response</dt><dd>{status.average_response_ms} ms ({status.response_samples})</dd></div>{/if}
                        {#if status?.cache_hits}<div><dt>Cache hits</dt><dd>{status.cache_hits}</dd></div>{/if}
                      </dl>
                      {#if status?.error}<small>{status.error}</small>{/if}
                    </div>
                  </div>
                {/each}
              </span>
            {/if}
          </span>
        {/if}
        {#if hasAmount && renderingRoutes}<small class="resultLimit" data-testid="route-render-progress">Showing {routes.length} now · loading more…</small>{/if}
        {#if selectedVenueId}<button type="button" class="venueFilterClear" on:click={() => selectedVenueId = null}>Showing {visibleRoutes.length} from {selectedVenueName} · Show all</button>{/if}
      </div>
      <div class="panelActions">
        {#if sourceCurrency && targetCurrency}
          <button type="button" class="activityButton" on:click={onOpenSearchActivity} aria-label={t("Open search activity graph", {}, $locale)} title={t("Open search activity graph", {}, $locale)}>
            <img src="/icons/ui/search-activity.png" alt="" width="18" height="18" aria-hidden="true" />
          </button>
        {/if}
        {#if searching && pendingVenues.length}
          <div class="searchingVenues" aria-label={`Searching ${pendingVenues.map((venue) => venue.label).join(", ")}`}>
            {#each visiblePendingVenues as venue, index (venue.id)}
              <span class="searchingVenue" data-testid="searching-venue" title={`Searching ${venue.label}`} style:animation-delay={`${index * 130}ms`}>
                <img src={venue.iconUrl} alt="" width="20" height="20" decoding="async" on:error={(event) => fallbackVenueIcon(event, venue.id)} />
              </span>
            {/each}
            {#if hasHiddenPendingVenues}<span class="searchingVenuesOverflow" data-testid="searching-venues-overflow" aria-hidden="true">...</span>{/if}
          </div>
        {/if}
        {#if showBelarusP2pWarning}
          <button type="button" class="legalWarning" on:click={onOpenBelarusP2pWarning} aria-label={t("Belarus P2P legal warning", {}, $locale)}>
            <img src={warningIcon} alt="" width="18" height="18" aria-hidden="true" />
          </button>
        {/if}
      </div>
    </div>
    {#if routes.length > 0}
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <div class="routeGroups" use:routeScrollColor data-testid="route-groups" role="region" tabindex="0" aria-label="Found routes">
        <ul class="routeList">
          {#each visibleRoutes as route, index (route.route_id)}
            {@const complete = route.status === "complete"}
            <li animate:flip={{ duration: routeFlipDuration, easing: quintOut }} in:fly={{ y: 18, duration: routeEnterDuration(), easing: quintOut }}><div class="routeCardShell">
              <div class:routeBest={route.is_current_best} class:selected={route.route_id === selectedRouteId} class="routeCard" data-testid={complete ? "complete-route" : "partial-route"}>
              <button type="button" class="routeCardMain" disabled={!complete} aria-pressed={route.route_id === selectedRouteId} aria-label={`Select route ${index + 1}: ${money(route.target_amount_minor, route.target_currency, route.target_amount)}`} on:click={(event) => cardClick(event, route)}>
                <span class="routeTopline"><span class="routeRank">#{String(index + 1).padStart(2, "0")}</span><span class="routeBadges">{#if route.is_current_best}<span class="bestBadge" data-testid="best-route-badge">{t("Best router", {}, $locale)}</span>{/if}{#if route.profitability}<span class={route.is_current_best ? "bestBadge" : "deltaBadge"}>{profitLabel(route)}</span>{:else if !route.is_current_best}<span class="deltaBadge">{spreadLabel(route.spread_bps, $locale)}</span>{/if}</span></span>
                <span class="routeAmount">{money(route.target_amount_minor, route.target_currency, route.target_amount)}</span>
              </button>
              <div class="routeActionRow">
                <button type="button" class="routeWorkflowButton" disabled={!complete} aria-label={workflowLabel(route)} on:click={(event) => cardClick(event, route)}>
                  <span class="workflow">
                  {#each workflowSteps(route) as step, stepIndex}
                    <span class="workflowPart">
                      {#if stepIndex > 0}<span class="workflowArrow" aria-hidden="true"><svg width="20" height="20" viewBox="0 0 24 24" fill="none"><path d="M3 12h16m-6-6 6 6-6 6" stroke="currentColor" stroke-width="2.8" stroke-linecap="round" stroke-linejoin="round" /></svg></span>{/if}
                      <span class="workflowControl">
                        <span class="workflowAsset">
                          <span class="workflowIcon" aria-hidden="true"><img class:fiatFlag={Boolean(fiatFlagUrl(step.currency))} src={fiatFlagUrl(step.currency) ?? (FIAT_CURRENCIES.has(step.currency.toUpperCase()) ? assetIcon(step.currency) : step.iconUrl ?? assetIcon(step.currency))} alt="" width="18" height="18" loading="lazy" decoding="async" /></span>
                          <strong>{step.currency.toUpperCase()}</strong>
                        </span>
                        {#if step.network}
                          <span class="workflowNetwork" aria-label={t("Network: {network}", { network: readableNetwork(step.network) }, $locale)} title={readableNetwork(step.network)}>
                            <span class="workflowNetworkIcon" aria-hidden="true"><img src={networkIcon(step.network)} alt="" width="18" height="18" loading="lazy" decoding="async" /></span>
                          </span>
                        {/if}
                        {#if step.paymentMethod}
                          <span class="workflowNetwork workflowPayment" aria-label={t("Payment method: {method}", { method: step.paymentMethod }, $locale)} title={step.paymentMethod}>
                            <span class="workflowNetworkIcon" aria-hidden="true">{#if step.paymentMethodIcon}<img src={step.paymentMethodIcon} alt="" width="18" height="18" loading="lazy" decoding="async" />{:else}<span class="paymentInitials">{step.paymentMethod.slice(0, 2).toUpperCase()}</span>{/if}</span>
                          </span>
                        {/if}
                        {#if step.provider}
                          <span class="workflowVenue" title={venueName(step.provider)}><span class="workflowVenueIcon" aria-hidden="true"><img src={venueIcon(step.provider)} alt="" width="18" height="18" loading="lazy" decoding="async" on:error={(event) => fallbackVenueIcon(event, step.provider ?? "")} /></span><strong>{venueName(step.provider)}</strong></span>
                        {/if}
                      </span>
                    </span>
                  {/each}
                  </span>
                </button>
                {#if complete && route.feedback}<div class="routeFeedback" aria-label="Route feedback"><span class="serviceVote routeVote"><button type="button" class:active={route.feedback.viewer_vote === "like"} aria-label="Like this route" aria-pressed={route.feedback.viewer_vote === "like"} title="Like this route" on:click={() => onVote(route, "like")}><img src={likeIcon} alt="" aria-hidden="true" />{#if route.feedback.viewer_vote}<span aria-label={`${compact(route.feedback.likes_total)} likes`}>{compact(route.feedback.likes_total)}</span>{/if}</button><button type="button" class:active={route.feedback.viewer_vote === "dislike"} aria-label="Dislike this route" aria-pressed={route.feedback.viewer_vote === "dislike"} title="Dislike this route" on:click={() => onVote(route, "dislike")}><img src={dislikeIcon} alt="" aria-hidden="true" />{#if route.feedback.viewer_vote}<span aria-label={`${compact(route.feedback.dislikes_total)} dislikes`}>{compact(route.feedback.dislikes_total)}</span>{/if}</button></span></div>{/if}
              </div>
              {#if route.route_fees?.length || route.quote_expires_at}
                <span class="routeQuoteMeta">
                  {#if route.route_fees?.length}Fee {route.route_fees.map((fee) => `${fee.amount} ${fee.asset}`).join(" + ")}{/if}
                  {#if route.quote_expires_at} · Quote expires {new Date(route.quote_expires_at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}{/if}
                </span>
              {/if}
              </div>
            </div></li>
          {/each}
        </ul>
        {#if selectedVenueId && visibleRoutes.length === 0}<p class="venueFilterEmpty">No ranked routes from {selectedVenueName} for this amount and payment method.</p>{/if}
      </div>
    {:else if searching}
      <div class="skeletonList" aria-label="Searching live routes">
        {#each [0, 1, 2, 3, 4] as item}
          <div class="skeletonCard" style:animation-delay={`${item * 80}ms`}><span class="skeletonShort"></span><span class="skeletonLong"></span><span class="skeletonMedium"></span></div>
        {/each}
      </div>
    {:else}
      <div class="emptyState">
        <div class="emptyVisual" aria-hidden="true"><span class="emptyNode">AM</span><span class="emptyPath"><i></i><i></i><i></i></span><span class="emptyNode">RU</span></div>
        <div><strong>{t(searched && hasAmount ? "No routes found" : hasAmount ? "Preparing market scan" : "Your routes will appear here", {}, $locale)}</strong><p>{t(searched && hasAmount ? circularSearch ? "No profitable quoted cycle was found for this amount and selected providers." : "No compatible live offers were found for this amount and payment method." : hasAmount ? "Pay3Flow is ready to compare entry assets, venues and recipient payout options." : "Enter an amount and we will assemble live cross-border paths in real time.", {}, $locale)}</p></div>
      </div>
    {/if}
  </div>
</aside>

<style>
.side {
  position: relative;
  z-index: 20;
  display: flex;
  min-width: 0;
  overflow: visible;
  font-family: var(--font-sans);
  opacity: 0;
  transform: translateX(10px);
  transition: opacity 0.35s ease, transform 0.35s ease;
}

.side.active {
  opacity: 1;
  transform: translateX(0);
}

.panel {
  position: relative;
  display: flex;
  width: 100%;
  height: 720px;
  min-width: 0;
  flex-direction: column;
  padding: 20px;
  overflow: visible;
  border: 1px solid rgba(255, 255, 255, 0.72);
  border-radius: var(--radius-card);
  background: #1b1e1a;
  box-shadow: none;
  color: #fff;
  isolation: isolate;
}

.panelTop,
.routeTopline,
.liveBadge,
.searchBadge,
.readyBadge,
.emptyVisual {
  display: flex;
  align-items: center;
}

.panelTop {
  position: relative;
  z-index: 40;
  min-height: 48px;
  align-items: flex-start;
  justify-content: space-between;
  gap: 18px;
}

.panelHeading {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 3px;
}

.panelActions {
  display: flex;
  align-items: flex-start;
  justify-content: flex-end;
  gap: 8px;
  flex: 0 0 auto;
}

.activityButton { display: none; width: 32px; height: 32px; flex: 0 0 auto; place-items: center; padding: 0; border: 0; border-radius: 9px; background: transparent; color: var(--color-text-soft); cursor: pointer; }
.activityButton img { display: block; width: 18px; height: 18px; object-fit: contain; filter: brightness(0); }
:global(html[data-theme="dark"]) .activityButton img { filter: brightness(0) invert(1); }
.activityButton:focus-visible { outline: var(--focus-ring-width) solid var(--color-focus); outline-offset: 2px; }
@media (max-width: 980px) { .activityButton { display: grid; } }

.resultSummary,
.foundVenues,
.searchingVenues {
  display: flex;
  align-items: center;
}

.resultSummary {
  min-height: 24px;
  flex-wrap: wrap;
  gap: 7px;
}


.legalWarning {
  display: grid;
  width: 30px;
  height: 30px;
  flex: 0 0 auto;
  place-items: center;
  padding: 0;
  border: 1px solid rgba(255, 199, 0, 0.42);
  border-radius: 9px;
  background: rgba(255, 199, 0, 0.12);
  cursor: pointer;
  transition: border-color 0.16s ease, background 0.16s ease, transform 0.16s ease;
}

.legalWarning:hover {
  border-color: #ffc700;
  border-width: var(--border-highlight-width);
  background: rgba(255, 199, 0, 0.2);
  transform: translateY(-1px);
}

.legalWarning img {
  filter: brightness(0) saturate(100%) invert(76%) sepia(91%) saturate(1160%) hue-rotate(355deg) brightness(101%) contrast(99%);
}

.legalWarning:focus-visible {
  outline: var(--focus-ring-width) solid var(--color-focus);
  outline-offset: 2px;
}

.foundVenues {
  position: relative;
  z-index: 41;
  gap: 8px;
  flex-wrap: wrap;
}

.foundVenue {
  position: relative;
  z-index: 1;
  display: grid;
  width: 32px;
  height: 32px;
  place-items: center;
  border: 1px solid #555b54;
  border-radius: 8px;
  background: #343834;
  animation: foundVenueIn 0.52s cubic-bezier(0.22, 1.42, 0.36, 1) both;
  will-change: transform, opacity;
}

.foundVenueButton {
  display: grid;
  width: 100%;
  height: 100%;
  place-items: center;
  padding: 0;
  border: 0;
  background: transparent;
  cursor: pointer;
}

.foundVenueActive {
  border-color: #94dd00;
  box-shadow: none; outline: var(--focus-ring-width) solid var(--color-accent-strong); outline-offset: 2px;
}

.venueFilterClear {
  width: fit-content;
  padding: 2px 0;
  border: 0;
  background: none;
  color: var(--color-accent-text);
  font-size: 12px;
  font-weight: 700;
  cursor: pointer;
}

.venueFilterEmpty {
  padding: 16px;
  color: var(--color-text-soft);
  font-size: 12px;
}

.foundVenue:hover,
.foundVenue:focus-within {
  z-index: 50;
}

.foundVenuePopover {
  position: absolute;
  top: calc(100% + 10px);
  left: 50%;
  z-index: 100;
  display: grid;
  width: min(240px, calc(100vw - 32px));
  box-sizing: border-box;
  gap: 7px;
  padding: 13px 14px;
  border: 1px solid var(--color-border-strong);
  border-radius: 14px;
  background: #fff;
  box-shadow: none;
  color: var(--color-text);
  font-size: 12px;
  opacity: 0;
  pointer-events: none;
  transform: translate(-50%, -4px);
  transition: opacity 0.16s ease, transform 0.16s ease;
}

.foundVenue:hover .foundVenuePopover,
.foundVenue:focus-within .foundVenuePopover {
  opacity: 1;
  pointer-events: auto;
  transform: translate(-50%, 0);
}

.foundVenuePopover > span {
  color: var(--color-text-soft);
  font-size: 12px;
}

.foundVenuePopover .venueOk { color: var(--color-accent-text); }
.foundVenuePopover .venueError { color: var(--color-danger); }

.foundVenuePopover dl {
  display: grid;
  gap: 5px;
  margin: 0;
}

.foundVenuePopover dl div {
  display: flex;
  justify-content: space-between;
  gap: 12px;
}

.foundVenuePopover dt { color: var(--color-text-soft); }
.foundVenuePopover dd { margin: 0; font-weight: 800; }
.foundVenuePopover small { color: var(--color-danger); line-height: 1.35; }

.foundVenuePopover strong {
  color: #1c2419;
  font-size: 12px;
}

.foundVenuePopoverTitle {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 8px;
}

.foundVenuePopoverTitle img {
  flex: 0 0 auto;
  width: 20px;
  height: 20px;
  border-radius: 6px;
  object-fit: contain;
}

.foundVenuePopover dt { color: #687464; }

.foundVenuePopover dd { color: #1c2419; }

.foundVenue img {
  width: 18px;
  height: 18px;
  border-radius: 6px;
  object-fit: contain;
}

.foundVenueFallback {
  color: #49630c;
  font-size: 12px;
  font-weight: 850;
  line-height: 1;
}

.panelTop strong {
  font-size: 16px;
  font-weight: 700;
  letter-spacing: -0.025em;
}

.panelTop small {
  color: rgba(255, 255, 255, 0.68);
  font-size: 12px;
}

.panelTop .resultLimit {
  color: rgba(255, 255, 255, 0.42);
  font-size: 12px;
}

.liveBadge,
.searchBadge,
.readyBadge {
  min-height: 30px;
  flex: 0 0 auto;
  gap: 7px;
  padding: 0 10px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: var(--radius-pill);
  background: rgba(255, 255, 255, 0.06);
  color: rgba(255, 255, 255, 0.62);
  font-size: 12px;
  font-weight: 800;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.liveBadge i,
.searchBadge i,
.readyBadge i {
  width: 6px;
  height: 6px;
  border-radius: 50%;
}

.liveBadge i {
  background: var(--color-accent);
  box-shadow: none;
}

.searchBadge i {
  background: #a28dff;
  box-shadow: none;
  animation: pulse 1s ease-in-out infinite;
}

.readyBadge i {
  background: rgba(255, 255, 255, 0.35);
}

.routeGroups {
  scrollbar-color: var(--route-scroll-color, #b5f500) var(--color-panel);
  min-height: 0;
  flex: 1;
  margin-top: 14px;
  padding-right: 6px;
  overflow-y: auto;
  overflow-anchor: none;
  overscroll-behavior: contain;
  scrollbar-gutter: stable;
  contain: layout paint;
  transform: translateZ(0);
}

.routeGroups:focus-visible {
  outline: var(--focus-ring-width) solid var(--color-focus);
  outline-offset: 4px;
  border-radius: 18px;
}

.routeList {
  display: flex;
  flex-direction: column;
  gap: 9px;
  margin: 0;
  padding: 0 0 2px;
  list-style: none;
}

.routeList > li {
  content-visibility: auto;
  contain-intrinsic-size: 0 132px;
  will-change: transform, opacity;
}

.routeCardShell {
  display: flex;
  position: relative;
  width: 100%;
  min-width: 0;
  flex-direction: column;
  gap: 7px;
}

.routeCard {
  position: relative;
  display: flex;
  width: 100%;
  box-sizing: border-box;
  flex-direction: column;
  gap: 8px;
  padding: 15px;
  overflow: hidden;
  border: 1px solid transparent;
  border-radius: 20px;
  background: transparent;
  color: #fff;
  text-align: left;
  transition: border-color 0.16s ease, background 0.16s ease, transform 0.16s ease;
}

.routeCardMain {
  display: flex;
  width: 100%;
  flex-direction: column;
  gap: 8px;
  padding: 0;
  border: 0;
  background: transparent;
  color: inherit;
  text-align: left;
}

.routeActionRow {
  display: flex;
  width: 100%;
  min-width: 0;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.routeWorkflowButton {
  display: block;
  min-width: 0;
  flex: 1 1 auto;
  padding: 0;
  border: 0;
  background: transparent;
  color: inherit;
  cursor: pointer;
  font: inherit;
  text-align: left;
}

.routeWorkflowButton:disabled {
  cursor: default;
  opacity: 1;
}

.routeFeedback {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 5px 12px;
  flex: 0 0 auto;
  margin: 0;
  color: rgba(255, 255, 255, 0.58);
  font-family: var(--font-mono);
  font-size: 12px;
}

.serviceVote {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}

.routeVote {
  padding: 2px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.04);
}

.serviceVoteName {
  overflow: hidden;
  font-weight: 750;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.serviceVote button {
  display: inline-flex;
  min-width: 32px;
  min-height: 32px;
  align-items: center;
  justify-content: center;
  gap: 3px;
  padding: 2px 4px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: inherit;
  font-family: inherit;
  font-size: 12px;
}

.serviceVote button:hover {
  background: rgba(255, 255, 255, 0.07);
}

.serviceVote button.active {
  background: var(--color-accent-soft);
  color: var(--color-accent-text);
}

.serviceVote img {
  width: 14px;
  height: 14px;
  object-fit: contain;
  filter: brightness(0) saturate(100%) invert(34%) sepia(9%) saturate(956%) hue-rotate(72deg) brightness(91%) contrast(88%);
}

.routeBest {
  border-color: rgba(185, 242, 39, 0.3);
  border-width: var(--border-highlight-width);
  background: linear-gradient(135deg, rgba(185, 242, 39, 0.095), rgba(255, 255, 255, 0.04));
}

.selected {
  border-color: var(--color-accent);
  border-width: var(--border-highlight-width);
  box-shadow: none;
}

.routeTopline,
.routeFooter {
  position: relative;
  z-index: 1;
  justify-content: space-between;
  gap: 10px;
}

.routeRank {
  color: rgba(255, 255, 255, 0.32);
  font-family: var(--font-mono);
  font-size: 12px;
}

.routeBadges {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: flex-end;
  gap: 5px;
}

.bestBadge,
.deltaBadge {
  padding: 4px 8px;
  border-radius: var(--radius-pill);
  font-size: 12px;
  font-weight: 850;
  letter-spacing: 0.07em;
  text-transform: uppercase;
}

.bestBadge {
  background: var(--color-accent);
  color: #171717;
}

.deltaBadge {
  background: rgba(255, 255, 255, 0.07);
  color: rgba(255, 255, 255, 0.52);
}

.routeAmount {
  position: relative;
  z-index: 1;
  overflow: hidden;
  padding-bottom: 10px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.1);
  cursor: pointer;
  font-family: var(--font-sans);
  font-size: 22px;
  font-weight: 700;
  letter-spacing: -0.05em;
  text-overflow: ellipsis;
  white-space: nowrap;
  transition: color 0.15s ease;
}

.routeAmount:hover {
  color: var(--color-accent-text);
}

.workflow {
  position: relative;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px 8px;
  padding-top: 1px;
  width: 100%;
  min-width: 0;
  z-index: 1;
  color: rgba(255, 255, 255, 0.55);
  font-size: 12px;
  line-height: 1;
}

.routeQuoteMeta {
  position: relative;
  z-index: 1;
  overflow: hidden;
  color: rgba(255, 255, 255, 0.42);
  font-size: 12px;
  line-height: 1.4;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.workflowPart,
.workflowAsset,
.workflowVenue {
  display: inline-flex;
  min-width: 0;
  align-items: center;
}

.workflowPart {
  max-width: 100%;
  gap: 6px;
}

.workflowControl {
  display: inline-flex;
  min-width: 0;
  max-width: 100%;
  align-items: stretch;
  overflow: hidden;
  border: 1px solid #d7dcd3;
  border-radius: 5px;
  background: #e7ebe2;
}

.workflowAsset {
  flex: 0 0 auto;
  gap: 6px;
  padding: 5px 8px 5px 6px;
  color: var(--color-text);
}

.workflowAsset strong {
  font-size: 12px;
  font-weight: 850;
}

.workflowNetwork {
  display: inline-flex;
  flex: 0 0 auto;
  align-items: center;
  padding: 5px 7px;
  border-left: 1px solid #d7dcd3;
}

.workflowNetworkIcon {
  display: inline-grid;
  width: 18px;
  height: 18px;
  flex: 0 0 auto;
  place-items: center;
  overflow: hidden;
  border-radius: 50%;
  background: rgba(255, 255, 255, 0.08);
}

.workflowNetworkIcon img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.workflowIcon,
.workflowVenueIcon {
  display: inline-grid;
  flex: 0 0 auto;
  place-items: center;
  overflow: hidden;
  border-radius: 50%;
}

.workflowIcon {
  width: 18px;
  height: 18px;
  background: rgba(255, 255, 255, 0.08);
}

.workflowVenueIcon {
  width: 18px;
  height: 18px;
}

.workflowIcon img,
.workflowVenueIcon img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.workflowIcon img.fiatFlag { object-fit: cover; }
.routeGroups::-webkit-scrollbar { width: 8px; }
.routeGroups::-webkit-scrollbar-track { background: var(--color-panel); }
.routeGroups::-webkit-scrollbar-thumb { background: var(--route-scroll-color, #b5f500); border-radius: 8px; }

.workflowVenue {
  flex: 0 1 auto;
  gap: 6px;
  padding: 5px 8px 5px 7px;
  border-left: 1px solid #d7dcd3;
  color: var(--color-text);
}

.workflowVenue strong {
  overflow: hidden;
  line-height: 1.5;
  padding-block: 1px;
  font-size: 12px;
  font-weight: 850;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.workflowArrow {
  display: inline-grid;
  width: 20px;
  height: 20px;
  flex: 0 0 auto;
  place-items: center;
  color: var(--color-text);
}

.paymentInitials {
  font-size: 9px;
  font-weight: 800;
  line-height: 1;
}

.workflowArrow svg {
  display: block;
  width: 100%;
  height: 100%;
}

.skeletonList {
  position: relative;
  display: flex;
  min-height: 0;
  flex: 1;
  flex-direction: column;
  gap: 9px;
  margin-top: 14px;
  overflow: hidden;
}

.searchingVenues {
  min-height: 34px;
  flex: 0 0 auto;
  justify-content: flex-end;
  gap: 6px;
  padding-top: 1px;
}

.searchingVenue {
  display: grid;
  width: 32px;
  height: 32px;
  place-items: center;
  border: 1px solid rgba(255, 255, 255, 0.14);
  border-radius: 11px;
  background: rgba(27, 30, 26, 0.92);
  box-shadow: none;
  animation: venueBounce 1.3s cubic-bezier(0.45, 0, 0.55, 1) infinite;
  will-change: transform;
}

.searchingVenuesOverflow {
  display: grid;
  width: 20px;
  height: 32px;
  flex: 0 0 auto;
  place-items: center;
  color: rgba(255, 255, 255, 0.58);
  font-weight: 800;
  letter-spacing: 0.08em;
}

.searchingVenue img {
  width: 20px;
  height: 20px;
  border-radius: 6px;
  object-fit: contain;
}

.skeletonCard {
  display: flex;
  min-height: 0;
  flex: 1 1 0;
  flex-direction: column;
  justify-content: center;
  gap: clamp(6px, 1vh, 12px);
  padding: clamp(10px, 1.3vh, 17px);
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 18px;
  background: rgba(255, 255, 255, 0.04);
  animation: skeletonPulse 1.2s ease-in-out infinite alternate;
}

.skeletonShort,
.skeletonLong,
.skeletonMedium {
  height: 8px;
  border-radius: 6px;
  background: rgba(255, 255, 255, 0.09);
}

.skeletonShort { width: 24%; }
.skeletonLong { width: 62%; height: 19px; }
.skeletonMedium { width: 78%; }

.emptyState {
  display: flex;
  min-height: 0;
  flex: 1;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 24px 18px 12px;
  text-align: center;
}

.emptyVisual {
  width: min(300px, 100%);
  justify-content: center;
  gap: 12px;
  margin-bottom: 29px;
}

.emptyNode {
  display: grid;
  width: 58px;
  height: 58px;
  flex: 0 0 auto;
  place-items: center;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 19px;
  background: rgba(255, 255, 255, 0.06);
  color: #fff;
  font-family: var(--font-mono);
  font-size: 12px;
}

.emptyNode:last-child {
  border-color: rgba(185, 242, 39, 0.18);
  background: rgba(185, 242, 39, 0.08);
  color: var(--color-accent);
}

.emptyPath {
  position: relative;
  display: flex;
  min-width: 90px;
  flex: 1;
  align-items: center;
  justify-content: space-around;
}

.emptyPath::before {
  position: absolute;
  right: 0;
  left: 0;
  height: 1px;
  background: linear-gradient(90deg, rgba(255,255,255,0.1), var(--color-accent), rgba(255,255,255,0.1));
  content: "";
}

.emptyPath i {
  position: relative;
  z-index: 1;
  width: 7px;
  height: 7px;
  border: 2px solid #1b1e1a;
  border-radius: 50%;
  background: var(--color-accent);
  animation: pathPulse 1.8s ease-in-out infinite;
}

.emptyPath i:nth-child(2) { animation-delay: 0.2s; }
.emptyPath i:nth-child(3) { animation-delay: 0.4s; }

.emptyState strong {
  font-size: 18px;
  font-weight: 700;
  letter-spacing: -0.03em;
}

.emptyState p {
  max-width: 330px;
  margin-top: 8px;
  color: rgba(255, 255, 255, 0.42);
  font-size: 12px;
  line-height: 1.65;
}

@keyframes pulse {
  0%, 100% { opacity: 0.5; transform: scale(0.8); }
  50% { opacity: 1; transform: scale(1.15); }
}

@keyframes skeletonPulse {
  from { opacity: 0.55; }
  to { opacity: 1; }
}

@keyframes venueBounce {
  0%, 45%, 100% { transform: translateY(0) scale(1); }
  18% { transform: translateY(-7px) scale(1.05); }
  28% { transform: translateY(1px) scale(0.98); }
}

@keyframes foundVenueIn {
  0% { opacity: 0; transform: translate(9px, -5px) scale(0.72); }
  72% { opacity: 1; transform: translate(-1px, 1px) scale(1.06); }
  100% { opacity: 1; transform: translate(0, 0) scale(1); }
}

@keyframes pathPulse {
  0%, 100% { opacity: 0.3; transform: scale(0.7); }
  50% { opacity: 1; transform: scale(1.15); }
}

@media (max-width: 980px) {
  .panel {
    height: 580px;
  }
}

@media (max-width: 640px) {
  .panel {
    height: 520px;
    padding: 15px;
    border-radius: 25px;
  }

  .workflow {
    white-space: normal;
  }
}

/* The route board follows the same paper-and-lime language as the exchange card. */
.panel {
  height: 690px;
  padding: 24px;
  border-color: var(--color-border-strong);
  border-radius: var(--radius-card);
  background: rgba(255, 255, 255, 0.96);
  box-shadow: none;
  color: var(--color-text);
}

.panelTop strong {
  color: var(--color-text);
}

.panelTop small,
.routeFeedback {
  color: var(--color-text-soft);
}

.routeVote {
  border-color: var(--color-border);
  background: #f4f8f1;
}

.panelTop {
  transform: translateY(-5px);
}

.liveBadge,
.searchBadge,
.readyBadge {
  border-color: var(--color-border);
  background: #f4f8f1;
  color: var(--color-text-soft);
}

.liveBadge i {
  box-shadow: none;
}

.searchBadge i {
  box-shadow: none;
}

.routeList {
  gap: 8px;
}

.routeCard {
  padding: 15px;
  border-color: var(--color-border);
  border-radius: 10px;
  background: #fbfcfa;
  color: var(--color-text);
  transition: border-color 0.16s ease, background 0.16s ease, transform 0.16s ease, box-shadow 0.16s ease;
}

.routeFeedback {
  color: var(--color-text-soft);
}

.serviceVoteName {
  color: var(--color-text-soft);
}

.serviceVote button:hover {
  background: var(--color-panel-soft);
  color: var(--color-text);
}

.serviceVote button.active {
  background: var(--color-accent-soft);
  color: var(--color-accent-text);
}

.routeBest {
  border-color: #b5d27d;
  border-width: var(--border-highlight-width);
  background: #f3f9e8;
}

.selected {
  border-color: var(--color-accent-text);
  box-shadow: none;
  border-width: var(--focus-ring-width);
}

.routeRank,
.workflow {
  color: var(--color-text-faint);
}

.workflowVenue {
  color: var(--color-text);
  font-weight: 750;
}

.bestBadge {
  background: var(--color-accent);
  color: #171717;
}

.deltaBadge {
  background: #eef4e9;
  color: var(--color-text-soft);
}

.routeAmount {
  color: var(--color-text);
  border-bottom-color: var(--color-border);
}

.skeletonCard {
  border-color: var(--color-border);
  background: #f4f8f1;
}

.searchingVenue {
  border-color: var(--color-border-strong);
  background: rgba(255, 255, 255, 0.94);
  box-shadow: none;
}

.foundVenue {
  border-color: #c6cac5;
  background: #e7e9e6;
}

.foundVenuePopover {
  border-color: #cbd0ca;
  background: #f1f2f1;
}

.skeletonShort,
.skeletonLong,
.skeletonMedium {
  background: #dfe9db;
}

.emptyNode {
  border-color: var(--color-border);
  background: #f4f8f1;
  color: var(--color-text-soft);
}

.emptyNode:last-child {
  border-color: #cce29a;
  background: #f1f8df;
  color: var(--color-accent-text);
}

.emptyPath::before {
  background: linear-gradient(90deg, #e0e9dc, var(--color-accent-strong), #e0e9dc);
}

.emptyPath i {
  border-color: #fff;
  background: var(--color-accent-strong);
}

.emptyState strong {
  color: var(--color-text);
}

.emptyState p {
  color: var(--color-text-faint);
}

@media (max-width: 640px) {
  .panel {
    height: 520px;
    padding: 16px;
  }

  .routeActionRow {
    gap: 5px;
  }

  .workflow {
    gap: 3px;
  }

  .workflow:has(> .workflowPart:nth-child(2):last-child) {
    flex-wrap: nowrap;
  }

  .workflow:has(> .workflowPart:nth-child(2):last-child) > .workflowPart:first-child {
    flex-shrink: 0;
  }

  .workflowPart {
    gap: 2px;
  }

  .workflowAsset {
    gap: 3px;
    padding: 4px 5px 4px 4px;
  }

  .workflowNetwork {
    padding: 4px 3px;
  }

  .workflowVenue {
    gap: 3px;
    padding: 4px 5px 4px 4px;
  }

  .workflowAsset strong,
  .workflowVenue strong {
    font-size: 12px;
  }

  .workflowIcon,
  .workflowNetworkIcon,
  .workflowVenueIcon {
    width: 12px;
    height: 12px;
  }

  .workflowArrow {
    width: 13px;
    height: 13px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .routeList > li,
  .routeCard,
  .searchingVenue,
  .foundVenue {
    animation: none;
    transition: none;
  }
}

:global(html[data-theme="dark"]) .panel {
  border-color: var(--color-border-strong);
  background: rgba(25, 25, 25, 0.96);
  box-shadow: none;
}

:global(html[data-theme="dark"]) .routeCard,
:global(html[data-theme="dark"]) .skeletonCard,
:global(html[data-theme="dark"]) .emptyNode {
  border-color: #383838;
  background: #202020;
}

:global(html[data-theme="dark"]) .routeFeedback,
:global(html[data-theme="dark"]) .serviceVoteName {
  color: rgba(255, 255, 255, 0.58);
}

:global(html[data-theme="dark"]) .routeVote {
  border-color: #383838;
  background: rgba(255, 255, 255, 0.04);
}

:global(html[data-theme="dark"]) .serviceVote button:hover {
  background: rgba(255, 255, 255, 0.07);
  color: var(--color-text);
}

:global(html[data-theme="dark"]) .serviceVote button.active {
  background: var(--color-accent-soft);
  color: var(--color-accent-text);
}

:global(html[data-theme="dark"]) .serviceVote img {
  filter: none;
}

:global(html[data-theme="dark"]) .searchingVenue {
  border-color: #454545;
  background: rgba(32, 32, 32, 0.96);
  box-shadow: none;
}

:global(html[data-theme="dark"]) .foundVenue {
  border-color: #575d56;
  background: #343834;
}

:global(html[data-theme="dark"]) .foundVenuePopover {
  border-color: #555b54;
  background: #2d302d;
  box-shadow: none;
  color: #f0f5ea;
}

:global(html[data-theme="dark"]) .foundVenuePopover strong,
:global(html[data-theme="dark"]) .foundVenuePopover dd {
  color: #f0f5ea;
}

:global(html[data-theme="dark"]) .foundVenuePopover > span,
:global(html[data-theme="dark"]) .foundVenuePopover dt {
  color: #aeb9a7;
}

:global(html[data-theme="dark"]) .foundVenueFallback {
  color: #d8f59a;
}


:global(html[data-theme="dark"]) .routeBest,
:global(html[data-theme="dark"]) .selected {
  border-color: rgba(181, 245, 0, 0.38);
  background: rgba(181, 245, 0, 0.08);
}

:global(html[data-theme="dark"]) .routeRank,
:global(html[data-theme="dark"]) .deltaBadge {
  background: #2b2b2b;
}

:global(html[data-theme="dark"]) .panelTop small,
:global(html[data-theme="dark"]) .routeFeedback {
  color: rgba(255, 255, 255, 0.58);
}

:global(html[data-theme="dark"]) .workflow,
:global(html[data-theme="dark"]) .workflowPart,
:global(html[data-theme="dark"]) .workflowAsset,
:global(html[data-theme="dark"]) .workflowVenue {
  background: transparent;
}

:global(html[data-theme="dark"]) .workflowControl {
  border-color: #3b3b3b;
  background: #2a2a2a;
}

:global(html[data-theme="dark"]) .workflowNetwork,
:global(html[data-theme="dark"]) .workflowVenue {
  border-color: #3b3b3b;
}

:global(html[data-theme="dark"]) .skeletonShort,
:global(html[data-theme="dark"]) .skeletonLong,
:global(html[data-theme="dark"]) .skeletonMedium {
  background: #383838;
}


.panelTop { transform: none; padding-bottom: 16px; border-bottom: 1px solid var(--color-border); }
.routeGroups { padding: 12px 6px 12px 0; border-bottom: 1px solid var(--color-border); }
.emptyState p { font-size: 14px; }
.routeQuoteMeta { overflow-wrap: anywhere; }
.foundVenuePopover::before { position: absolute; height: 12px; left: 0; right: 0; bottom: 100%; content: ""; }
:global(html[data-theme="dark"]) .venueFilterClear,
:global(html[data-theme="dark"]) .foundVenuePopover .venueOk { color: var(--color-accent-text); }
@media (max-width: 980px), (pointer: coarse) {
  .activityButton, .legalWarning { width: 44px; height: 44px; }
  .panelActions { margin-right: -4px; }
  .foundVenues { gap: 12px; padding: 6px 0; }
  .foundVenue { width: 32px; height: 32px; }
  .foundVenueButton { position: absolute; width: 44px; height: 44px; min-width: 44px; min-height: 44px; }
  .serviceVote button { min-width: 44px; min-height: 44px; }
  .routeCard { gap: 12px; padding: 12px; }
  .routeTopline { align-items: start; gap: 8px; }
  .routeBadges { flex: 1; min-width: 0; justify-content: flex-end; }
  .bestBadge, .deltaBadge { max-width: 100%; white-space: normal; overflow-wrap: anywhere; text-align: right; line-height: 1.45; }
  .routeAmount { max-width: 100%; font-size: clamp(22px, 6vw, 28px); overflow-wrap: anywhere; }
  .routeActionRow { flex-direction: column; align-items: stretch; gap: 10px; }
  .routeWorkflowButton { width: 100%; flex: initial; }
  .routeFeedback { justify-content: flex-end; padding-top: 8px; border-top: 1px solid var(--color-border); }
  .routeVote { gap: 8px; padding: 0; border: 0; background: transparent; }
  .workflowPart { width: 100%; gap: 8px; }
  .workflowControl { flex: 1; }
  .workflowPart:first-child { padding-left: 28px; }
  .workflowArrow { width: 20px; flex: 0 0 20px; }
  .workflowVenue { min-width: 0; margin-left: auto; }
  .workflowVenue strong { max-width: 110px; }
  .workflow { flex-wrap: wrap; gap: 8px; }
  .workflow:has(> .workflowPart:nth-child(2):last-child) { flex-wrap: wrap; }
  .workflowIcon, .workflowNetworkIcon, .workflowVenueIcon { width: 16px; height: 16px; }
  .panelTop { gap: 12px; }
  .panelHeading { flex: 1; }
  .resultSummary { gap: 8px; }
}
</style>
