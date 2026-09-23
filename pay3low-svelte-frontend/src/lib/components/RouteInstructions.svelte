<script lang="ts">
  import { onMount } from "svelte";
  import type { RouteCandidate, ServiceLink, ServiceStats, ServiceVote } from "$lib/exchange";
  import { dislikeIcon, likeIcon } from "$lib/icons";
  import AdvertiserCard from "./AdvertiserCard.svelte";

  export let route: RouteCandidate;
  export let onClose: () => void;
  export let usedServiceIds: string[] = [];
  export let onOpenService: (link: ServiceLink) => void = () => {};
  export let onVote: (service: ServiceStats, vote: ServiceVote | null) => void = () => {};
  let modal: HTMLDivElement;
  let dragging = false;
  let dragStartY = 0;
  let dragDistance = 0;
  const VENUE_NAMES: Record<string, string> = { binance: "Binance", bitget: "Bitget", bybit: "Bybit", okx: "OKX", rapira: "Rapira" };
  const venueName = (value?: string) => value ? VENUE_NAMES[value.toLowerCase()] ?? value : "P2P market";
  const money = (minor?: number, currency?: string) => minor == null ? "—" : `${(minor / 100).toLocaleString("en-US", { maximumFractionDigits: 2 })} ${currency ?? ""}`;
  const marketRate = (value: string) => Number.isFinite(Number(value)) ? Number(value).toLocaleString("en-US", { maximumFractionDigits: 12, useGrouping: false }) : value;
  const compact = (value: number) => Intl.NumberFormat("en", { notation: "compact", maximumFractionDigits: 1 }).format(value);
  const linkFor = (kind: ServiceLink["kind"]) => route.service_links?.find((link) => link.kind === kind);
  const chooseVote = (service: ServiceStats, vote: ServiceVote) => onVote(service, service.viewer_vote === vote ? null : vote);

  function spotPair(symbol: string, firstAsset: string, secondAsset: string) {
    const normalized = symbol.replace(/[^a-z0-9]/gi, "").toUpperCase();
    const first = firstAsset.toUpperCase(), second = secondAsset.toUpperCase();
    if (normalized === `${first}${second}`) return { base: first, quote: second };
    if (normalized === `${second}${first}`) return { base: second, quote: first };
    return null;
  }
  function spotUrl(venue: string, symbol: string, first: string, second: string) {
    const pair = spotPair(symbol, first, second), key = venue.toLowerCase();
    if (!pair) return ({ binance: "https://www.binance.com/en/trade", bybit: "https://www.bybit.com/trade/spot/", okx: "https://www.okx.com/trade-spot/", bitget: "https://www.bitget.com/spot/" } as Record<string, string>)[key] ?? null;
    if (key === "binance") return `https://www.binance.com/en/trade/${pair.base}_${pair.quote}?type=spot`;
    if (key === "bybit") return `https://www.bybit.com/trade/spot/${pair.base}/${pair.quote}`;
    if (key === "okx") return `https://www.okx.com/trade-spot/${pair.base.toLowerCase()}-${pair.quote.toLowerCase()}`;
    if (key === "bitget") return `https://www.bitget.com/spot/${pair.base}${pair.quote}`;
    return null;
  }
  function backdrop(event: MouseEvent) { if (event.target === event.currentTarget) onClose(); }
  function startSheetDrag(event: PointerEvent) {
    dragging = true;
    dragStartY = event.clientY;
    dragDistance = 0;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  }
  function moveSheetDrag(event: PointerEvent) {
    if (!dragging) return;
    dragDistance = Math.max(0, event.clientY - dragStartY);
    modal?.style.setProperty("--sheet-drag", `${dragDistance}px`);
  }
  function endSheetDrag() {
    if (!dragging) return;
    const shouldClose = dragDistance > 96 || (modal && dragDistance > modal.clientHeight * 0.24);
    dragging = false;
    if (shouldClose) {
      onClose();
    } else {
      modal?.style.removeProperty("--sheet-drag");
    }
  }
  onMount(() => {
    const handler = (event: KeyboardEvent) => event.key === "Escape" && onClose();
    const previousOverflow = document.body.style.overflow;
    const previousOverscrollBehavior = document.body.style.overscrollBehavior;
    document.body.style.overflow = "hidden";
    document.body.style.overscrollBehavior = "none";
    document.addEventListener("keydown", handler);
    return () => {
      document.body.style.overflow = previousOverflow;
      document.body.style.overscrollBehavior = previousOverscrollBehavior;
      document.removeEventListener("keydown", handler);
    };
  });
  $: entry = route.legs.find((leg) => leg.kind === "entry");
  $: exit = route.legs.find((leg) => leg.kind === "exit");
  $: entryVenue = venueName(entry?.provider);
  $: exitVenue = venueName(exit?.provider);
  $: crossVenue = Boolean(entry && exit && entry.provider !== exit.provider);
  $: cryptoToCrypto = route.route_kind === "crypto_to_crypto";
  $: transferNetwork = route.entry_network && route.entry_network.toLowerCase() !== "internal" ? route.entry_network : null;
  $: transferStepNumber = route.entry_offer_snapshot ? 2 : 1;
  $: exitStepNumber = (route.entry_offer_snapshot ? 1 : 0) + (crossVenue ? 1 : 0) + 1;
  $: firstMarketUrl = route.market_path ? spotUrl(route.market_path.venue, route.market_path.source_pair, route.source_currency, route.bridge_currency ?? route.target_currency ?? route.entry_asset) : null;
  $: secondMarketUrl = route.market_path && route.bridge_currency ? spotUrl(route.market_path.venue, route.market_path.target_pair, route.bridge_currency, route.target_currency ?? route.entry_asset) : null;
</script>

<div class="backdrop" role="presentation" on:mousedown={backdrop}>
  <div class:dragging class="modal" bind:this={modal} role="dialog" aria-modal="true" aria-labelledby="route-instructions-title" tabindex="-1">
    <button type="button" class="sheetHandle" aria-label="Close instructions by dragging down" on:pointerdown={startSheetDrag} on:pointermove={moveSheetDrag} on:pointerup={endSheetDrag} on:pointercancel={endSheetDrag}><span aria-hidden="true"></span></button>
    <div class="header"><div><span class="eyebrow">Selected route</span><h2 id="route-instructions-title">How to complete this exchange</h2><p class="intro">Complete each step in order. You stay in control—Pay3Flow never places an order or moves your funds.</p><p class="estimate">Estimated output: <strong>{money(route.target_amount_minor, route.target_currency)}</strong></p></div><button type="button" class="closeButton" on:click={onClose} aria-label="Close instructions">×</button></div>
    <ol class="workflow" aria-label="Exchange steps">
      {#if cryptoToCrypto && route.market_path}
        {@const market = route.market_path}
        <li class="step" data-testid="instruction-step"><span class="stepNumber" aria-hidden="true">1</span><div class="stepBody">
          <h3>{route.bridge_currency ? `Convert ${route.source_currency} to ${route.bridge_currency}` : `Convert ${route.source_currency} to ${route.target_currency}`}</h3>
          <p class="stepSummary">Use the {market.source_pair} spot market on {venueName(market.venue)}. This is an exchange order book, so there is no P2P advertiser to contact.</p>
          <ul class="checklist">
            <li>Confirm the pair converts {route.source_currency} into {route.bridge_currency ?? route.target_currency}.</li>
            <li>Review the live price, trading fee, and expected amount before submitting the order.</li>
            <li>Wait until the converted balance is available before continuing.</li>
          </ul>
          <div class="counterparty"><div class="counterpartyTopline"><span class="counterpartyLabel">Spot market</span><span class="profileBadge">{venueName(market.venue)}</span></div><strong class="advertiser">{market.source_pair}</strong><span class="venueLine">Conversion rate {marketRate(market.source_rate)}</span>
            {#if linkFor("market_source")}<button type="button" class="profileLink" on:click={() => onOpenService(linkFor("market_source")!)}>Open {market.source_pair} on {venueName(market.venue)} <span>↗</span></button>{:else if firstMarketUrl}<a href={firstMarketUrl} target="_blank" rel="noreferrer noopener" class="profileLink">Open {market.source_pair} on {venueName(market.venue)} <span>↗</span></a>{/if}
          </div>
        </div></li>
        {#if route.bridge_currency}
          <li class="step" data-testid="instruction-step"><span class="stepNumber" aria-hidden="true">2</span><div class="stepBody">
            <h3>Convert {route.bridge_currency} to {route.target_currency}</h3>
            <p class="stepSummary">Complete the second conversion on {venueName(market.venue)} only after the first trade has settled into your available balance.</p>
            <ul class="checklist">
              <li>Open {market.target_pair} and confirm it converts {route.bridge_currency} into {route.target_currency}.</li>
              <li>Review the live price, trading fee, and final amount before submitting the order.</li>
              <li>Confirm the destination asset and network before withdrawing it from the venue.</li>
            </ul>
            <div class="counterparty"><div class="counterpartyTopline"><span class="counterpartyLabel">Spot market</span><span class="profileBadge">{venueName(market.venue)}</span></div><strong class="advertiser">{market.target_pair}</strong><span class="venueLine">Conversion rate {marketRate(market.target_rate)}</span>
              {#if linkFor("market_target")}<button type="button" class="profileLink" on:click={() => onOpenService(linkFor("market_target")!)}>Open {market.target_pair} on {venueName(market.venue)} <span>↗</span></button>{:else if secondMarketUrl}<a href={secondMarketUrl} target="_blank" rel="noreferrer noopener" class="profileLink">Open {market.target_pair} on {venueName(market.venue)} <span>↗</span></a>{/if}
            </div>
          </div></li>
        {/if}
      {/if}
      {#if route.entry_offer_snapshot}
        <li class="step" data-testid="instruction-step"><span class="stepNumber" aria-hidden="true">1</span><div class="stepBody">
          <h3>{cryptoToCrypto ? `Sell ${route.source_currency} for ${route.bridge_currency ?? route.entry_asset}` : `Buy ${route.entry_asset} for ${money(route.source_amount_minor, route.source_currency)}`}</h3>
          <p class="stepSummary">{cryptoToCrypto ? `Open the buyer's profile on ${entryVenue} and create the first P2P order.` : `Open the seller's profile on ${entryVenue}, create the P2P order, and pay with the selected payment method.`}</p>
          <ul class="checklist">
            <li>Match the advertiser nickname and ad ID before creating the order.</li>
            <li>Confirm the live rate, order limits, and payment method on {entryVenue}.</li>
            <li>{cryptoToCrypto ? "Release the asset only after you have independently confirmed receipt of the payment." : "Use only the payment details shown inside the order, then mark it paid after sending the transfer."}</li>
          </ul>
          <AdvertiserCard offer={route.entry_offer_snapshot} label={`${cryptoToCrypto ? "Buyer" : "Seller"} on ${entryVenue}`} serviceLink={linkFor("entry")} {onOpenService} />
        </div></li>
      {/if}
      {#if crossVenue}
        <li class="step" data-testid="instruction-step"><span class="stepNumber" aria-hidden="true">{transferStepNumber}</span><div class="stepBody">
          <h3>Transfer {route.entry_asset} to {exitVenue}</h3>
          <p class="stepSummary">Move the purchased asset from {entryVenue} to your deposit address on {exitVenue} before opening the next P2P order.</p>
          <ul class="checklist">
            <li>{#if transferNetwork}Copy the deposit address from {exitVenue} and select the exact {transferNetwork} network on both venues.{:else}Confirm that both venues support the same asset and network, then copy the deposit address from {exitVenue}.{/if}</li>
            <li>Check the full address, any required memo or tag, and the withdrawal fee before confirming.</li>
            <li>Wait for {exitVenue} to credit the deposit before continuing.</li>
          </ul>
        </div></li>
      {/if}
      {#if route.exit_offer_snapshot}
        <li class="step" data-testid="instruction-step"><span class="stepNumber" aria-hidden="true">{exitStepNumber}</span><div class="stepBody">
          <h3>{cryptoToCrypto ? `Buy ${route.target_currency} with ${route.bridge_currency ?? route.entry_asset}` : `Sell ${route.entry_asset} for ${money(route.target_amount_minor, route.target_currency)}`}</h3>
          <p class="stepSummary">{cryptoToCrypto ? `Open the seller's profile on ${exitVenue} and create the destination-asset order.` : `Open the buyer's profile on ${exitVenue} and create the sell order using the selected recipient payment method.`}</p>
          <ul class="checklist">
            <li>Match the advertiser nickname and ad ID before creating the order.</li>
            <li>Confirm the live rate, order limits, {cryptoToCrypto ? "asset network" : "recipient payment method"}, and expected amount.</li>
            <li>{cryptoToCrypto ? `Confirm the ${route.target_currency} balance and network before withdrawing.` : "Release the asset only after you have independently confirmed the payment in your bank or payment account."}</li>
          </ul>
          <AdvertiserCard offer={route.exit_offer_snapshot} label={`${cryptoToCrypto ? "Seller" : "Buyer"} on ${exitVenue}`} serviceLink={linkFor("exit")} {onOpenService} />
        </div></li>
      {/if}
    </ol>
    {#if route.services?.length}<section class="serviceReputation" aria-label="Service reputation">{#each route.services as service (service.id)}<article><strong>{service.display_name}</strong><div class="serviceMetrics"><span>Used {compact(service.executions_total)} times</span><span class="reputationMetric" aria-label={`${compact(service.likes_total)} likes`}><img src={likeIcon} alt="" aria-hidden="true" />{compact(service.likes_total)}</span><span class="reputationMetric" aria-label={`${compact(service.dislikes_total)} dislikes`}><img src={dislikeIcon} alt="" aria-hidden="true" />{compact(service.dislikes_total)}</span></div>{#if usedServiceIds.includes(service.id) || service.viewer_vote}<div class="votePrompt"><span>Was this service useful?</span><div><button type="button" class:active={service.viewer_vote === "like"} aria-pressed={service.viewer_vote === "like"} on:click={() => chooseVote(service, "like")}><img src={likeIcon} alt="" aria-hidden="true" />Like</button><button type="button" class:active={service.viewer_vote === "dislike"} aria-pressed={service.viewer_vote === "dislike"} on:click={() => chooseVote(service, "dislike")}><img src={dislikeIcon} alt="" aria-hidden="true" />Dislike</button></div></div>{/if}</article>{/each}</section>{/if}
    <div class="warning"><strong>Important</strong><span>Rates, limits and ads can change. Confirm the user, payment details and network on the exchange before sending money. Pay3Flow never creates the order or moves funds.</span>{#each route.warnings ?? [] as warning}<span>{warning}</span>{/each}</div>
  </div>
</div>

<style>
.backdrop {
  position: fixed;
  inset: 0;
  z-index: 1200;
  display: grid;
  place-items: center;
  padding: 20px;
  background: rgba(15, 17, 14, 0.68);
  backdrop-filter: blur(18px) saturate(120%);
  -webkit-backdrop-filter: blur(18px) saturate(120%);
  animation: fadeIn 0.2s ease;
  touch-action: none;
}

.modal {
  width: min(100%, 560px);
  max-height: min(720px, calc(100vh - 40px));
  overflow-y: auto;
  scrollbar-width: none;
  padding: 28px;
  border: 1px solid rgba(255, 255, 255, 0.72);
  border-radius: 28px;
  background: rgba(250, 250, 246, 0.98);
  box-shadow: 0 35px 110px rgba(0, 0, 0, 0.3);
  color: var(--color-text);
  animation: modalIn 0.3s cubic-bezier(0.22, 1, 0.36, 1);
  touch-action: auto;
  overscroll-behavior: contain;
  transform: translateY(var(--sheet-drag, 0));
  transition: transform 0.24s ease;
}

.modal.dragging {
  transition: none;
}

.sheetHandle {
  display: none;
}

.modal::-webkit-scrollbar {
  display: none;
}

.header {
  display: flex;
  justify-content: space-between;
  gap: 20px;
}

.eyebrow {
  color: var(--color-violet);
  font-size: 9px;
  font-weight: 850;
  letter-spacing: 0.1em;
  text-transform: uppercase;
}

.header h2 {
  margin: 8px 0 7px;
  font-size: 27px;
  letter-spacing: -0.055em;
  line-height: 1.05;
}

.header p {
  margin: 0;
  color: var(--color-text-soft);
  font-size: 12px;
  line-height: 1.5;
}

.header p strong {
  color: var(--color-text);
}

.header .intro {
  max-width: 420px;
}

.header .estimate {
  margin-top: 8px;
}

.closeButton {
  width: 36px;
  height: 36px;
  flex: 0 0 auto;
  border: 1px solid var(--color-border);
  border-radius: 12px;
  color: var(--color-text-soft);
  font-size: 24px;
  line-height: 1;
}

.closeButton:hover {
  background: var(--color-accent-soft);
  color: var(--color-text);
}

.workflow {
  display: grid;
  gap: 0;
  margin: 28px 0 0;
  padding: 0;
  list-style: none;
}

.step {
  position: relative;
  display: grid;
  grid-template-columns: 42px minmax(0, 1fr);
  gap: 16px;
  padding: 0 0 30px;
}

.step:last-child {
  padding-bottom: 0;
}

.step:not(:last-child)::before {
  position: absolute;
  top: 40px;
  bottom: 0;
  left: 19px;
  width: 2px;
  border-radius: 999px;
  background: #d9e5d1;
  content: "";
}

.stepNumber {
  position: relative;
  z-index: 1;
  display: grid;
  width: 40px;
  height: 40px;
  place-items: center;
  border: 1px solid #76951e;
  border-radius: 50%;
  background: var(--color-primary);
  color: var(--color-accent);
  font-family: var(--font-mono);
  font-size: 13px;
  font-weight: 800;
  box-shadow: 0 0 0 5px rgba(181, 224, 58, 0.12);
}

.stepBody {
  min-width: 0;
  padding-top: 2px;
}

.step h3 {
  margin: 0;
  font-size: 16px;
  letter-spacing: -0.025em;
  line-height: 1.3;
}

.step .stepSummary {
  margin: 7px 0 0;
  color: var(--color-text-soft);
  font-size: 12px;
  line-height: 1.55;
}

.checklist {
  display: grid;
  gap: 7px;
  margin: 13px 0 0;
  padding: 0;
  color: var(--color-text-soft);
  font-size: 11px;
  line-height: 1.5;
  list-style: none;
}

.checklist li {
  position: relative;
  padding-left: 19px;
}

.checklist li::before {
  position: absolute;
  top: 0.12em;
  left: 0;
  display: grid;
  width: 14px;
  height: 14px;
  place-items: center;
  border-radius: 50%;
  background: rgba(109, 152, 0, 0.13);
  color: #5f8308;
  content: "✓";
  font-size: 9px;
  font-weight: 900;
  line-height: 1;
}

.counterparty {
  margin-top: 12px;
  padding: 12px;
  border: 1px solid rgba(111, 83, 190, 0.2);
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.78);
}

.counterpartyTopline {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.counterpartyIdentity {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.counterpartyIdentityCopy {
  min-width: 0;
  flex: 1;
}

.counterpartyAvatar {
  position: relative;
  display: grid;
  width: 42px;
  height: 42px;
  flex: 0 0 42px;
  place-items: center;
  border: 1px solid #cfe0bf;
  border-radius: 50%;
  background: linear-gradient(145deg, #d9f28c, #8dbd2b);
  color: #30430b;
  font-size: 16px;
  font-weight: 850;
  box-shadow: 0 5px 12px rgba(55, 77, 52, 0.12);
}

.avatarVenue {
  position: absolute;
  right: -3px;
  bottom: -2px;
  display: grid;
  width: 16px;
  height: 16px;
  place-items: center;
  overflow: hidden;
  border: 2px solid #f4f8f1;
  border-radius: 50%;
  background: #fff;
}

.avatarVenue img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.counterpartyLabel {
  color: var(--color-violet);
  font-size: 9px;
  font-weight: 850;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.profileBadge,
.manualBadge {
  padding: 4px 7px;
  border-radius: 999px;
  font-size: 8px;
  font-weight: 850;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.profileBadge {
  background: rgba(181, 224, 58, 0.2);
  color: #557309;
}

.manualBadge {
  background: rgba(240, 166, 89, 0.16);
  color: #8a5b1e;
}

.advertiser {
  display: block;
  margin-top: 9px;
  font-size: 16px;
  letter-spacing: -0.03em;
}

.counterpartyIdentityCopy .advertiser {
  margin-top: 6px;
}

.venueLine,
.paymentLine,
.adHint {
  display: block;
  color: var(--color-text-faint);
  font-size: 10px;
  line-height: 1.45;
}

.metrics {
  display: flex;
  flex-wrap: wrap;
  gap: 6px 12px;
  margin: 10px 0 7px;
  color: var(--color-text-soft);
  font-family: var(--font-mono);
  font-size: 9px;
}

.metrics b {
  color: var(--color-text);
}

.profileLink {
  display: inline-block;
  margin-top: 10px;
  padding: 0;
  border: 0;
  background: transparent;
  color: #587b08;
  font-size: 10px;
  font-weight: 850;
  text-decoration: none;
}

.serviceReputation {
  display: grid;
  gap: 10px;
  margin-top: 16px;
}

.serviceReputation article {
  padding: 14px;
  border: 1px solid var(--color-border);
  border-radius: 16px;
  background: rgba(255, 255, 255, 0.72);
}

.serviceReputation article > strong {
  font-size: 13px;
}

.serviceMetrics {
  display: flex;
  flex-wrap: wrap;
  gap: 6px 14px;
  margin-top: 7px;
  color: var(--color-text-soft);
  font-family: var(--font-mono);
  font-size: 10px;
}

.reputationMetric {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.reputationMetric img {
  width: 16px;
  height: 16px;
  object-fit: contain;
}

.votePrompt {
  margin-top: 12px;
  padding-top: 12px;
  border-top: 1px solid var(--color-border);
  color: var(--color-text-soft);
  font-size: 11px;
}

.votePrompt > div {
  display: flex;
  gap: 8px;
  margin-top: 8px;
}

.votePrompt button {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 7px 10px;
  border: 1px solid var(--color-border);
  border-radius: 10px;
  background: #fff;
  color: var(--color-text-soft);
  font-size: 10px;
}

.votePrompt button img {
  width: 16px;
  height: 16px;
  object-fit: contain;
}

.votePrompt button.active {
  border-color: rgba(111, 83, 190, 0.4);
  background: rgba(111, 83, 190, 0.1);
  color: var(--color-violet);
}

.profileLink:hover {
  text-decoration: underline;
}

.profileLink span {
  margin-left: 3px;
}

.adHint {
  margin-top: 6px;
  font-size: 9px;
}

.missing {
  color: var(--color-text-soft);
}

.warning {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-top: 16px;
  padding: 12px 14px;
  border: 1px solid rgba(240, 166, 89, 0.28);
  border-radius: 14px;
  background: rgba(240, 166, 89, 0.1);
  color: #75501f;
  font-size: 10px;
  line-height: 1.45;
}

.warning strong {
  font-size: 10px;
  text-transform: uppercase;
}

@keyframes fadeIn {
  from { opacity: 0; }
  to { opacity: 1; }
}

@keyframes modalIn {
  from { opacity: 0; transform: translateY(16px) scale(0.98); }
  to { opacity: 1; transform: translateY(0) scale(1); }
}

.backdrop {
  background: rgba(38, 57, 37, 0.28);
  backdrop-filter: blur(9px);
  -webkit-backdrop-filter: blur(9px);
}

.modal {
  padding: 24px;
  border-color: var(--color-border-strong);
  border-radius: 16px;
  background: rgba(255, 255, 255, 0.98);
  box-shadow: 0 30px 80px rgba(44, 64, 42, 0.18);
}

.eyebrow,
.counterpartyLabel {
  color: #6d9800;
}

.closeButton {
  border-color: var(--color-border);
  border-radius: 9px;
}

.counterparty {
  border-color: #d4e4c5;
  background: #f4f8f1;
}

@media (max-width: 560px) {
  .backdrop {
    align-items: end;
    padding: 0;
  }

  .modal {
    width: 100%;
    max-height: min(90vh, 760px);
    padding: 8px 18px max(18px, env(safe-area-inset-bottom));
    border-radius: 23px 23px 0 0;
    transform: translateY(var(--sheet-drag, 0));
  }

  .sheetHandle {
    display: flex;
    width: 100%;
    height: 30px;
    align-items: center;
    justify-content: center;
    color: var(--color-text-faint);
    cursor: grab;
    touch-action: none;
    user-select: none;
  }

  .sheetHandle:active {
    cursor: grabbing;
  }

  .sheetHandle span {
    display: block;
    width: 38px;
    height: 5px;
    border-radius: 999px;
    background: currentColor;
  }

  .header h2 {
    font-size: 23px;
  }

  .workflow {
    margin-top: 24px;
  }

  .step {
    grid-template-columns: 36px minmax(0, 1fr);
    gap: 12px;
    padding-bottom: 26px;
  }

  .step:not(:last-child)::before {
    top: 35px;
    left: 16px;
  }

  .stepNumber {
    width: 34px;
    height: 34px;
    font-size: 11px;
  }

  .step h3 {
    font-size: 15px;
  }
}

:global(html[data-theme="dark"]) .modal {
  border-color: var(--color-border-strong);
  background: rgba(25, 25, 25, 0.99);
  box-shadow: var(--shadow-pop);
}

:global(html[data-theme="dark"]) .closeButton,
:global(html[data-theme="dark"]) .counterparty {
  border-color: #3b3b3b;
  background: #222222;
}

:global(html[data-theme="dark"]) .step:not(:last-child)::before {
  background: #3c4435;
}

:global(html[data-theme="dark"]) .stepNumber {
  border-color: #91b52b;
  box-shadow: 0 0 0 5px rgba(181, 224, 58, 0.08);
}

:global(html[data-theme="dark"]) .checklist li::before {
  background: rgba(181, 224, 58, 0.12);
  color: var(--color-accent);
}

:global(html[data-theme="dark"]) .serviceReputation article {
  border-color: #3b3b3b;
  background: #222222;
  color: var(--color-text);
}

:global(html[data-theme="dark"]) .votePrompt button {
  border-color: #464646;
  background: #2b2b2b;
  color: var(--color-text);
}

:global(html[data-theme="dark"]) .votePrompt button:hover {
  border-color: #606060;
  background: #333333;
}

:global(html[data-theme="dark"]) .votePrompt button.active {
  border-color: rgba(162, 141, 255, 0.55);
  background: rgba(162, 141, 255, 0.16);
  color: var(--color-violet);
}

:global(html[data-theme="dark"]) .avatarVenue {
  border-color: #222222;
  background: #ffffff;
}

:global(html[data-theme="dark"]) .profileBadge {
  color: var(--color-accent);
}

</style>
