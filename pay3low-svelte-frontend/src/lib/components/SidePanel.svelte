<script lang="ts">
  import { flip } from "svelte/animate";
  import { quintOut } from "svelte/easing";
  import { fly } from "svelte/transition";
  import type { RouteCandidate } from "$lib/exchange";
  import { assetIcon, dislikeIcon, likeIcon, networkIcon, venueIcon } from "$lib/icons";

  export let routes: RouteCandidate[];
  export let routesFound = 0;
  export let sourceCurrency = "";
  export let targetCurrency = "";
  export let selectedRouteId: string | null;
  export let onSelect: (route: RouteCandidate) => void;
  export let onOpenInstructions: (route: RouteCandidate) => void;
  export let searching = false;
  export let searchingVenues: { id: string; label: string; iconUrl: string }[] = [];
  export let foundVenues: { id: string; label: string; iconUrl: string }[] = [];
  export let searched = false;
  export let hasAmount = false;

  const ASSET_NAMES: Record<string, string> = { BTC: "Bitcoin", ETH: "Ether", USDC: "USD Coin", USDT: "Tether" };
  const VENUE_NAMES: Record<string, string> = { binance: "Binance", bitget: "Bitget", bybit: "Bybit", okx: "OKX", rapira: "Rapira", whitebird: "Whitebird" };
  const VENUE_ICONS: Record<string, string> = {
    binance: venueIcon("binance"), bybit: venueIcon("bybit"), okx: venueIcon("okx"), bitget: venueIcon("bitget"), rapira: venueIcon("rapira"), whitebird: venueIcon("whitebird"),
  };
  const FIAT_MARKS: Record<string, string> = { AMD: "🇦🇲", RUB: "🇷🇺", BYN: "🇧🇾" };
  type Step = { currency: string; network?: string; provider?: string; iconUrl?: string };

  const venueName = (value?: string) => value ? VENUE_NAMES[value.toLowerCase()] ?? value : "Searching";
  const assetLabel = (currency?: string) => !currency ? "—" : ASSET_NAMES[currency.toUpperCase()] ? `${currency} ${ASSET_NAMES[currency.toUpperCase()]}` : currency;
  const money = (minor?: number, currency?: string) => minor == null ? "—" : `${(minor / 100).toLocaleString("en-US", { maximumFractionDigits: 2, useGrouping: false })} ${currency ?? ""}`;
  const spreadLabel = (bps: number) => Math.abs(bps / 100) < 0.005 ? "Same output" : `${Math.abs(bps / 100).toFixed(2)}% less`;
  const compact = (value: number) => Intl.NumberFormat("en", { notation: "compact", maximumFractionDigits: 1 }).format(value);
  const routeCountLabel = (count: number) => `${count} ${count === 1 ? "route" : "routes"} found`;
  const reduceMotion = () => typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const routeFlipDuration = (distance: number) => reduceMotion() ? 0 : Math.min(680, 260 + distance * 0.65);
  const routeEnterDuration = () => reduceMotion() ? 0 : 380;
  $: pendingVenues = searchingVenues.filter((venue) => !foundVenues.some((found) => found.id === venue.id));

  function workflowSteps(route: RouteCandidate): Step[] {
    const entry = route.legs.find((leg) => leg.kind === "entry");
    const exit = route.legs.find((leg) => leg.kind === "exit");
    const source = route.source_currency ?? "—";
    const target = route.target_currency ?? "—";
    if (!entry && exit) return [{ currency: source, network: route.source_network, provider: exit.provider, iconUrl: route.source_method_icon_url }, { currency: target, iconUrl: route.target_method_icon_url }];
    if (entry && !exit) return [{ currency: source, network: route.source_network, iconUrl: route.source_method_icon_url }, { currency: target, network: route.target_network, provider: entry.provider, iconUrl: route.target_method_icon_url }];
    if (route.bridge_currency) return [{ currency: source, network: route.source_network, provider: entry?.provider, iconUrl: route.source_method_icon_url }, { currency: route.bridge_currency }, { currency: target, network: route.target_network, provider: exit?.provider, iconUrl: route.target_method_icon_url }];
    return [{ currency: source, iconUrl: route.source_method_icon_url }, { currency: route.entry_asset ?? "—", network: route.entry_network !== "internal" ? route.entry_network : undefined, provider: entry?.provider }, { currency: target, provider: exit?.provider, iconUrl: route.target_method_icon_url }];
  }

  const workflowLabel = (route: RouteCandidate) => workflowSteps(route).map((step) => `${assetLabel(step.currency)}${step.network ? ` · ${step.network}` : ""}${step.provider ? ` (${venueName(step.provider)})` : ""}`).join(" → ");

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
</script>

<aside class="side active" aria-label="Found routes" aria-busy={searching} id="routes">
  <div class="panel">
    <div class="panelTop">
      <div class="panelHeading">
        {#if hasAmount}
          <strong>Send {sourceCurrency} → {targetCurrency}</strong>
          <span class="resultSummary">
            <small aria-live="polite">{routeCountLabel(routesFound)}{searching ? " · searching…" : ""}</small>
            {#if foundVenues.length}
              <span class="foundVenues" aria-label={`Routes found on ${foundVenues.map((venue) => venue.label).join(", ")}`}>
                {#each foundVenues as venue, index (venue.id)}
                  <span class="foundVenue" data-testid="found-venue" title={`Found on ${venue.label}`} style:animation-delay={`${index * 70}ms`}>
                    <img src={venue.iconUrl} alt="" width="18" height="18" decoding="async" on:error={(event) => fallbackVenueIcon(event, venue.id)} />
                  </span>
                {/each}
              </span>
            {/if}
          </span>
          {#if routesFound > routes.length && routes.length}<small class="resultLimit">Showing top {routes.length}</small>{/if}
        {:else}<strong>Awaiting your intent</strong>{/if}
      </div>
      {#if searching && pendingVenues.length}
        <div class="searchingVenues" aria-label={`Searching ${pendingVenues.map((venue) => venue.label).join(", ")}`}>
          {#each pendingVenues as venue, index (venue.id)}
            <span class="searchingVenue" data-testid="searching-venue" title={`Searching ${venue.label}`} style:animation-delay={`${index * 130}ms`}>
              <img src={venue.iconUrl} alt="" width="20" height="20" decoding="async" on:error={(event) => fallbackVenueIcon(event, venue.id)} />
            </span>
          {/each}
        </div>
      {/if}
    </div>
    {#if routes.length > 0}
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <div class="routeGroups" data-testid="route-groups" role="region" tabindex="0" aria-label="Found routes">
        <ul class="routeList">
          {#each routes as route, index (route.route_id)}
            {@const complete = route.status === "complete"}
            <li animate:flip={{ duration: routeFlipDuration, easing: quintOut }} in:fly={{ y: 18, duration: routeEnterDuration(), easing: quintOut }}><div class="routeCardShell">
              <button type="button" class:routeBest={route.is_current_best} class:selected={route.route_id === selectedRouteId} class="routeCard" disabled={!complete} on:click={(event) => cardClick(event, route)} data-testid={complete ? "complete-route" : "partial-route"}>
                <span class="routeTopline"><span class="routeRank">#{String(index + 1).padStart(2, "0")}</span>{#if route.is_current_best}<span class="bestBadge">Best route</span>{:else}<span class="deltaBadge">{spreadLabel(route.spread_bps)}</span>{/if}</span>
                <span class="routeAmount">{money(route.target_amount_minor, route.target_currency)}</span>
                <span class="workflow" aria-label={workflowLabel(route)}>
                  {#each workflowSteps(route) as step, stepIndex}
                    <span class="workflowPart">
                      {#if stepIndex > 0}<span class="workflowArrow" aria-hidden="true">→</span>{/if}
                      <span class="workflowAsset">
                        {#if FIAT_MARKS[step.currency.toUpperCase()] && !step.iconUrl}
                          <span class="workflowFlag" aria-hidden="true">{FIAT_MARKS[step.currency.toUpperCase()]}</span>
                        {:else}
                          <span class="workflowIcon" aria-hidden="true"><img src={step.iconUrl ?? assetIcon(step.currency)} alt="" width="15" height="15" loading="lazy" decoding="async" /></span>
                        {/if}
                        <span>{assetLabel(step.currency)}</span>
                        {#if step.network}
                          <span class="workflowNetwork" aria-label={`Network: ${step.network}`}>
                            <span aria-hidden="true">·</span>
                            <span class="workflowNetworkIcon" aria-hidden="true"><img src={networkIcon(step.network)} alt="" width="14" height="14" loading="lazy" decoding="async" /></span>
                            <span>{step.network}</span>
                          </span>
                        {/if}
                      </span>
                      {#if step.provider}
                        <span class="workflowVenue"><span aria-hidden="true">(</span>{#if VENUE_ICONS[step.provider.toLowerCase()]}<span class="workflowVenueIcon" aria-hidden="true"><img src={VENUE_ICONS[step.provider.toLowerCase()]} alt="" width="12" height="12" loading="lazy" decoding="async" on:error={(event) => fallbackVenueIcon(event, step.provider ?? "")} /></span>{/if}<span>{venueName(step.provider)}</span><span aria-hidden="true">)</span></span>
                      {/if}
                    </span>
                  {/each}
                </span>
                {#if route.reputation}<span class="routeReputation"><span>Used {compact(route.reputation.executions_average)} times</span><span class="reputationMetric" aria-label={`${compact(route.reputation.likes_average)} likes`}><img src={likeIcon} alt="" aria-hidden="true" />{compact(route.reputation.likes_average)}</span><span class="reputationMetric" aria-label={`${compact(route.reputation.dislikes_average)} dislikes`}><img src={dislikeIcon} alt="" aria-hidden="true" />{compact(route.reputation.dislikes_average)}</span></span>{/if}
              </button>
            </div></li>
          {/each}
        </ul>
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
        <div><strong>{searched && hasAmount ? "No routes found" : hasAmount ? "Preparing market scan" : "Your routes will appear here"}</strong><p>{searched && hasAmount ? "No compatible live offers were found for this amount and payment method." : hasAmount ? "Pay3Flow is ready to compare entry assets, venues and recipient payout options." : "Enter an amount and we will assemble live cross-border paths in real time."}</p></div>
        <div class="emptyVenues"><span>BINANCE</span><span>BYBIT</span><span>OKX</span><span>BITGET</span><span>RAPIRA</span></div>
      </div>
    {/if}
  </div>
</aside>

<style>
.side {
  position: relative;
  z-index: 1;
  display: flex;
  min-width: 0;
  overflow: hidden;
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
  overflow: hidden;
  border: 1px solid rgba(255, 255, 255, 0.72);
  border-radius: var(--radius-card);
  background: #1b1e1a;
  box-shadow: 0 28px 75px rgba(22, 25, 21, 0.18);
  color: #fff;
  isolation: isolate;
}

.panelTop,
.routeTopline,
.liveBadge,
.searchBadge,
.readyBadge,
.emptyVisual,
.emptyVenues {
  display: flex;
  align-items: center;
}

.panelTop {
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

.foundVenues {
  gap: 4px;
}

.foundVenue {
  display: grid;
  width: 24px;
  height: 24px;
  place-items: center;
  border: 1px solid rgba(185, 242, 39, 0.34);
  border-radius: 8px;
  background: rgba(185, 242, 39, 0.1);
  animation: foundVenueIn 0.52s cubic-bezier(0.22, 1.42, 0.36, 1) both;
  will-change: transform, opacity;
}

.foundVenue img {
  width: 18px;
  height: 18px;
  border-radius: 6px;
  object-fit: contain;
}

.panelTop strong {
  font-size: 16px;
  font-weight: 700;
  letter-spacing: -0.025em;
}

.panelTop small {
  color: rgba(255, 255, 255, 0.68);
  font-size: 11px;
}

.panelTop .resultLimit {
  color: rgba(255, 255, 255, 0.42);
  font-size: 9px;
}

.routeReputation {
  display: flex;
  flex-wrap: wrap;
  gap: 5px 12px;
  margin-top: 10px;
  color: rgba(255, 255, 255, 0.58);
  font-family: var(--font-mono);
  font-size: 9px;
}

.reputationMetric {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.reputationMetric img {
  width: 14px;
  height: 14px;
  object-fit: contain;
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
  font-size: 8px;
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
  box-shadow: 0 0 0 4px rgba(185, 242, 39, 0.1);
}

.searchBadge i {
  background: #a28dff;
  box-shadow: 0 0 0 4px rgba(162, 141, 255, 0.1);
  animation: pulse 1s ease-in-out infinite;
}

.readyBadge i {
  background: rgba(255, 255, 255, 0.35);
}

.routeGroups {
  min-height: 0;
  flex: 1;
  margin-top: 14px;
  padding-right: 6px;
  overflow-y: auto;
  overscroll-behavior: contain;
  scrollbar-width: none;
  contain: layout paint;
  transform: translateZ(0);
}

.routeGroups::-webkit-scrollbar {
  display: none;
}

.routeGroups:focus-visible {
  outline: 2px solid rgba(185, 242, 39, 0.55);
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

.routeBest {
  border-color: rgba(185, 242, 39, 0.3);
  background: linear-gradient(135deg, rgba(185, 242, 39, 0.095), rgba(255, 255, 255, 0.04));
}

.selected {
  border-color: var(--color-accent);
  box-shadow: inset 0 0 0 1px var(--color-accent);
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
  font-size: 8px;
}

.bestBadge,
.deltaBadge {
  padding: 4px 8px;
  border-radius: var(--radius-pill);
  font-size: 7px;
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
  color: #6d9800;
}

.workflow {
  position: relative;
  display: flex;
  align-items: center;
  gap: 4px;
  padding-top: 1px;
  width: 100%;
  min-width: 0;
  z-index: 1;
  overflow: hidden;
  color: rgba(255, 255, 255, 0.55);
  font-size: 9px;
  font-weight: 650;
  line-height: 1.5;
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
  gap: 3px;
}

.workflowAsset {
  gap: 4px;
}

.workflowNetwork {
  display: inline-flex;
  min-width: 0;
  align-items: center;
  gap: 3px;
}

.workflowNetworkIcon {
  display: inline-grid;
  width: 14px;
  height: 14px;
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
  width: 15px;
  height: 15px;
  background: rgba(255, 255, 255, 0.08);
}

.workflowVenueIcon {
  width: 12px;
  height: 12px;
  margin: 0 2px;
}

.workflowIcon img,
.workflowVenueIcon img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.workflowFlag {
  display: inline-block;
  flex: 0 0 auto;
  font-size: 13px;
  line-height: 1;
}

.workflowVenue {
  color: rgba(255, 255, 255, 0.45);
}

.workflowArrow {
  flex: 0 0 auto;
  color: rgba(255, 255, 255, 0.32);
}

.skeletonList {
  position: relative;
  display: flex;
  min-height: 0;
  flex: 1;
  flex-direction: column;
  gap: 9px;
  margin-top: 14px;
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
  box-shadow: 0 8px 18px rgba(9, 12, 8, 0.18);
  animation: venueBounce 1.3s cubic-bezier(0.45, 0, 0.55, 1) infinite;
  will-change: transform;
}

.searchingVenue img {
  width: 20px;
  height: 20px;
  border-radius: 6px;
  object-fit: contain;
}

.skeletonCard {
  display: flex;
  min-height: 116px;
  flex-direction: column;
  gap: 12px;
  padding: 17px;
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
  font-size: 11px;
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
  font-size: 10px;
  line-height: 1.65;
}

.emptyVenues {
  flex-wrap: wrap;
  justify-content: center;
  gap: 6px;
  margin-top: 26px;
}

.emptyVenues span {
  padding: 6px 8px;
  border: 1px solid rgba(255, 255, 255, 0.07);
  border-radius: var(--radius-pill);
  color: rgba(255, 255, 255, 0.32);
  font-size: 7px;
  font-weight: 850;
  letter-spacing: 0.08em;
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
  box-shadow: var(--shadow-card);
  color: var(--color-text);
}

.panelTop strong {
  color: var(--color-text);
}

.panelTop small,
.routeReputation {
  color: var(--color-text-soft);
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
  box-shadow: 0 0 0 4px rgba(45, 142, 69, 0.1);
}

.searchBadge i {
  box-shadow: 0 0 0 4px rgba(117, 88, 246, 0.09);
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

.routeBest {
  border-color: #b5d27d;
  background: #f3f9e8;
}

.selected {
  border-color: var(--color-accent-strong);
  box-shadow: inset 0 0 0 1px var(--color-accent-strong), 0 8px 18px rgba(141, 203, 0, 0.1);
}

.routeRank,
.workflow {
  color: var(--color-text-faint);
}

.workflowVenue {
  color: var(--color-text-soft);
  font-weight: 750;
}

.workflowArrow {
  color: var(--color-text-faint);
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
  box-shadow: 0 14px 34px rgba(22, 25, 21, 0.14);
}

.foundVenue {
  border-color: #cce29a;
  background: #f1f8df;
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
  color: #6d9800;
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

.emptyState p,
.emptyVenues span {
  color: var(--color-text-faint);
}

.emptyVenues span {
  border-color: var(--color-border);
}

@media (max-width: 640px) {
  .panel {
    height: 520px;
    padding: 16px;
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
  box-shadow: var(--shadow-card);
}

:global(html[data-theme="dark"]) .routeCard,
:global(html[data-theme="dark"]) .skeletonCard,
:global(html[data-theme="dark"]) .emptyNode {
  border-color: #383838;
  background: #202020;
}

:global(html[data-theme="dark"]) .searchingVenue {
  border-color: #454545;
  background: rgba(32, 32, 32, 0.96);
  box-shadow: 0 14px 34px rgba(0, 0, 0, 0.3);
}

:global(html[data-theme="dark"]) .foundVenue {
  border-color: rgba(181, 245, 0, 0.34);
  background: rgba(181, 245, 0, 0.09);
}

:global(html[data-theme="dark"]) .routeBest,
:global(html[data-theme="dark"]) .selected {
  border-color: rgba(181, 245, 0, 0.38);
  background: rgba(181, 245, 0, 0.08);
}

:global(html[data-theme="dark"]) .routeRank,
:global(html[data-theme="dark"]) .deltaBadge,
:global(html[data-theme="dark"]) .emptyVenues span {
  background: #2b2b2b;
}

:global(html[data-theme="dark"]) .panelTop small,
:global(html[data-theme="dark"]) .routeReputation {
  color: rgba(255, 255, 255, 0.58);
}

:global(html[data-theme="dark"]) .workflow,
:global(html[data-theme="dark"]) .workflowPart,
:global(html[data-theme="dark"]) .workflowAsset,
:global(html[data-theme="dark"]) .workflowVenue {
  background: transparent;
}

:global(html[data-theme="dark"]) .skeletonShort,
:global(html[data-theme="dark"]) .skeletonLong,
:global(html[data-theme="dark"]) .skeletonMedium {
  background: #383838;
}

</style>
