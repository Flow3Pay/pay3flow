<script lang="ts">
  import { onMount } from "svelte";
  import { lockPageScroll } from "$lib/page-scroll-lock";
  import type { ProviderGuidance, RouteCandidate, ServiceLink } from "$lib/exchange";
  import { locale, t } from "$lib/i18n";
  import AdvertiserCard from "./AdvertiserCard.svelte";
  import RouteExecutionPanel from "./RouteExecutionPanel.svelte";

  export let route: RouteCandidate;
  export let venueNames: Record<string, string> = {};
  export let providerGuidance: Record<string, ProviderGuidance> = {};
  export let networkNames: Record<string, string> = {};
  export let onClose: () => void;
  export let onOpenService: (link: ServiceLink) => void = () => {};
  let modal: HTMLDivElement;
  let dragging = false;
  let dragStartY = 0;
  let dragDistance = 0;
  const venueName = (value?: string | null) => value ? venueNames[value.toLowerCase()] ?? value : "P2P market";
  const providerGuide = (value?: string | null) => value ? providerGuidance[value.toLowerCase()] : undefined;
  const guideSteps = (guide: ProviderGuidance | undefined, side: "buy" | "sell") => [
    ...(guide?.steps ?? []),
    ...(side === "buy" ? guide?.buy_steps ?? [] : guide?.sell_steps ?? []),
  ];
  const readableNetwork = (value: string) => networkNames[value.toLowerCase()] ?? value.replace(/-/g, " ").replace(/\b\w/g, (letter) => letter.toUpperCase());
  const readablePath = (path: string[]) => path.map((part) => {
    const [asset, network] = part.split("@", 2);
    return network ? `${asset} in ${readableNetwork(network)}` : asset;
  }).join(" → ");
  $: language = $locale;
  const copy = (key: string, params: Record<string, string | number> = {}) => t(key, params, language);
  const isDirectOffer = (offer?: RouteCandidate["entry_offer_snapshot"]) => offer?.advertiser.user_type === "service" || offer?.source.toLowerCase() === "whitebird";
  const money = (minor?: number, currency?: string, exact?: string) => {
    if (exact && ["BTC", "ETH", "USDC", "USDT", "SOL", "TRX", "TON", "XRP", "ADA", "AVAX", "DOT", "LINK", "LTC", "BCH", "BNB", "DOGE", "MATIC", "NEAR", "SUI", "APT", "ATOM", "UNI", "DAI", "FDUSD"].includes(currency?.toUpperCase() ?? "")) {
      return `${Number(exact).toLocaleString("en-US", { maximumFractionDigits: 8 })} ${currency ?? ""}`;
    }
    return minor == null ? "—" : `${(minor / 100).toLocaleString("en-US", { maximumFractionDigits: 2 })} ${currency ?? ""}`;
  };
  const marketRate = (value: string) => Number.isFinite(Number(value)) ? Number(value).toLocaleString("en-US", { maximumFractionDigits: 12, useGrouping: false }) : value;
  const linkFor = (kind: ServiceLink["kind"]) => route.service_links?.find((link) => link.kind === kind);
  const bankFeeLabel = (bank?: string, percent?: number) => {
    if (!bank || percent == null) return null;
    return percent === 0 ? copy("{bank}: no bank fee", { bank }) : copy("{bank}: {percent}% bank fee", { bank, percent: percent.toFixed(2) });
  };

  function spotPair(symbol: string, firstAsset: string, secondAsset: string) {
    const normalized = symbol.replace(/[^a-z0-9]/gi, "").toUpperCase();
    const first = firstAsset.toUpperCase(), second = secondAsset.toUpperCase();
    if (normalized === `${first}${second}`) return { base: first, quote: second };
    if (normalized === `${second}${first}`) return { base: second, quote: first };
    return null;
  }
  function spotUrl(venue: string, symbol: string, first: string, second: string) {
    const pair = spotPair(symbol, first, second), key = venue.toLowerCase();
    if (!pair) return ({ binance: "https://www.binance.com/en/trade", bybit: "https://www.bybit.com/trade/spot/", okx: "https://www.okx.com/trade-spot/", bitget: "https://www.bitget.com/spot/", mexc: "https://www.mexc.com/exchange/" } as Record<string, string>)[key] ?? null;
    if (key === "binance") return `https://www.binance.com/en/trade/${pair.base}_${pair.quote}?type=spot`;
    if (key === "bybit") return `https://www.bybit.com/trade/spot/${pair.base}/${pair.quote}`;
    if (key === "okx") return `https://www.okx.com/trade-spot/${pair.base.toLowerCase()}-${pair.quote.toLowerCase()}`;
    if (key === "bitget") return `https://www.bitget.com/spot/${pair.base}${pair.quote}`;
    if (key === "mexc") return `https://www.mexc.com/exchange/${pair.base}_${pair.quote}`;
    if (key === "cifra-broker") return "https://tradernet.by/authentication/signup";
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
    const unlockPage = lockPageScroll();
    document.addEventListener("keydown", handler);
    return () => {
      unlockPage();
      document.removeEventListener("keydown", handler);
    };
  });
  $: entry = route.legs.find((leg) => leg.kind === "entry");
  $: exit = route.legs.find((leg) => leg.kind === "exit");
  $: entryVenue = venueName(entry?.provider);
  $: exitVenue = venueName(exit?.provider);
  $: entryDirect = isDirectOffer(route.entry_offer_snapshot);
  $: exitDirect = isDirectOffer(route.exit_offer_snapshot);
  $: directFiat = route.route_kind === "fiat_to_fiat" && entryDirect && !route.exit_offer_snapshot && !route.route_provider;
  $: crossVenue = Boolean(entry && exit && !route.route_provider && entry.provider !== exit.provider);
  $: cryptoToCrypto = route.route_kind === "crypto_to_crypto" || Boolean(route.cycle_legs?.length);
  $: providerSwap = Boolean(route.route_provider && (route.entry_offer_snapshot || route.exit_offer_snapshot));
  $: transferNetwork = route.entry_network && route.entry_network.toLowerCase() !== "internal" ? route.entry_network : null;
  $: entryAsset = route.entry_offer_snapshot?.asset ?? route.entry_asset;
  $: providerSwapFrom = route.entry_offer_snapshot?.asset ?? route.source_currency;
  $: providerSwapTo = route.exit_offer_snapshot?.asset ?? route.target_currency;
  $: sourceFeeLabel = language && bankFeeLabel(route.source_payment_method, route.source_bank_fee_percent);
  $: targetFeeLabel = language && bankFeeLabel(route.target_payment_method, route.target_bank_fee_percent);
  $: firstMarketUrl = route.market_path ? spotUrl(route.market_path.venue, route.market_path.source_pair, route.source_currency, route.bridge_currency ?? route.target_currency ?? route.entry_asset) : null;
  $: secondMarketUrl = route.market_path && route.bridge_currency ? spotUrl(route.market_path.venue, route.market_path.target_pair, route.bridge_currency, route.target_currency ?? route.entry_asset) : null;
  $: standaloneProvider = Boolean(route.route_provider && !providerSwap);
  $: routeGuide = providerGuide(route.route_provider);
  $: entryGuide = providerGuide(entry?.provider ?? route.entry_offer_snapshot?.source);
  $: exitGuide = providerGuide(exit?.provider ?? route.exit_offer_snapshot?.source);
  $: entryGuideSteps = guideSteps(entryGuide, cryptoToCrypto ? "sell" : "buy");
  $: exitGuideSteps = guideSteps(exitGuide, cryptoToCrypto ? "buy" : "sell");
  $: marketStepCount = cryptoToCrypto && route.market_path ? (route.bridge_currency ? 2 : 1) : 0;
  $: entryStepNumber = (standaloneProvider ? 1 : 0) + marketStepCount + 1;
  $: providerStepNumber = entryStepNumber + (route.entry_offer_snapshot ? 1 : 0);
  $: transferStepNumber = providerStepNumber + (providerSwap ? 1 : 0);
  $: exitStepNumber = transferStepNumber + (crossVenue ? 1 : 0);
  const warningText = (warning: string) => {
    let match = warning.match(/^Live dry quote from (.+); execution and wallet compatibility are not verified\.$/);
    if (match) return copy("Live dry quote from {provider}; execution and wallet compatibility are not verified.", { provider: match[1] });
    match = warning.match(/^Quoted exchanger: (.+)\.$/);
    if (match) return copy("Quoted exchanger: {description}.", { description: match[1] });
    return copy(warning);
  };
</script>

<div class="backdrop" role="presentation" on:mousedown={backdrop}>
  <div class:dragging class="modal" bind:this={modal} role="dialog" aria-modal="true" aria-labelledby="route-instructions-title" tabindex="-1">
    <button type="button" class="sheetHandle" aria-label={copy("Close instructions by dragging down")} on:pointerdown={startSheetDrag} on:pointermove={moveSheetDrag} on:pointerup={endSheetDrag} on:pointercancel={endSheetDrag}><span aria-hidden="true"></span></button>
    <div class="header"><div><span class="eyebrow">{copy("Selected route")}</span><h2 id="route-instructions-title">{copy("How to complete this exchange")}</h2><p class="intro">{copy("Complete each step in order. You stay in control—Pay3Flow never places an order or moves your funds.")}</p><p class="estimate">{copy("Estimated output:")} <strong>{money(route.target_amount_minor, route.target_currency, route.target_amount)}</strong></p>{#if sourceFeeLabel || targetFeeLabel}<p class="feeSummary">{copy("Bank fees:")} {[sourceFeeLabel, targetFeeLabel].filter(Boolean).join(" · ")}</p>{/if}</div><button type="button" class="closeButton" on:click={onClose} aria-label={copy("Close instructions")}>×</button></div>
    <ol class="workflow" aria-label={copy("Exchange steps")}>
      {#if route.cycle_legs?.length}
        {#each route.cycle_legs as leg, index}
          <li class="step" data-testid="instruction-step"><span class="stepNumber" aria-hidden="true">{index + 1}</span><div class="stepBody">
            <h3>{copy("Convert {from} to {to}", { from: readablePath([leg.from_asset]), to: readablePath([leg.to_asset]) })}</h3>
            <p class="stepSummary">{copy("Route through {venue}", { venue: venueName(leg.provider) })}</p>
            <p class="routePath">{leg.input_amount} {leg.from_asset.split("@")[0]} → {leg.output_amount} {leg.to_asset.split("@")[0]}</p>
            <ul class="checklist"><li>{copy("Check the current price, fee, and amount you should receive before pressing the exchange button.")}</li><li>{copy("Wait until the new balance appears before doing the next step.")}</li><li>{copy("Never send money after the quote expires. Get a new quote first.")}</li></ul>
            {#if leg.source_url}<a href={leg.source_url} target="_blank" rel="noreferrer noopener" class="profileLink">{copy("Open {venue}", { venue: venueName(leg.provider) })} <span>↗</span></a>{/if}
          </div></li>
        {/each}
      {:else if route.route_provider && !providerSwap}
        <li class="step" data-testid="instruction-step"><span class="stepNumber" aria-hidden="true">1</span><div class="stepBody">
          <h3>{copy("Route through {venue}", { venue: venueName(route.route_provider) })}</h3>
          <p class="stepSummary">{copy("This is a current estimate only. Pay3Flow does not send money or make the exchange for you.")}</p>
          {#if route.route_path?.length}<p class="routePath">{readablePath(route.route_path)}</p>{/if}
          {#if routeGuide}
            <div class="providerGuide">
              <p>{routeGuide.description}</p>
              {#if routeGuide.steps.length}<ul class="checklist">{#each routeGuide.steps as step}<li>{step}</li>{/each}</ul>{/if}
              {#if routeGuide.links.length}<div class="guideLinks">{#each routeGuide.links as link}<a href={link.url} target="_blank" rel="noreferrer noopener" class="profileLink">{link.label} <span>↗</span></a>{/each}</div>{/if}
            </div>
          {/if}
          <ul class="checklist">
            <li>{copy("Check which asset and network you send, and which asset and network you receive.")}</li>
            <li>{copy("Check the amount you will receive, the provider fee, how long the quote is valid, and whether a memo or tag is required.")}</li>
            <li>{copy("Never send money after the quote expires. Get a new quote first.")}</li>
          </ul>
          {#if route.route_provider_url}<a href={route.route_provider_url} target="_blank" rel="noreferrer noopener" class="profileLink">{copy("Open {venue}", { venue: venueName(route.route_provider) })} <span>↗</span></a>{/if}
          <RouteExecutionPanel {route} />
        </div></li>
      {/if}
      {#if cryptoToCrypto && route.market_path}
        {@const market = route.market_path}
        <li class="step" data-testid="instruction-step"><span class="stepNumber" aria-hidden="true">1</span><div class="stepBody">
          <h3>{copy("Convert {from} to {to}", { from: route.source_currency, to: route.bridge_currency ?? route.target_currency ?? "" })}</h3>
          <p class="stepSummary">{copy("This is a normal exchange on {venue}. There is no separate person to message.", { venue: venueName(market.venue) })}</p>
          <ul class="checklist">
            <li>{copy("First check that the pair changes {from} into {to}.", { from: route.source_currency, to: route.bridge_currency ?? route.target_currency ?? "" })}</li>
            <li>{copy("Check the current price, fee, and amount you should receive before pressing the exchange button.")}</li>
            <li>{copy("Wait until the new balance appears before doing the next step.")}</li>
          </ul>
          <div class="counterparty"><div class="counterpartyTopline"><span class="counterpartyLabel">{copy("Spot market")}</span><span class="profileBadge">{venueName(market.venue)}</span></div><strong class="advertiser">{market.source_pair}</strong><span class="venueLine">{copy("Conversion rate {rate}", { rate: marketRate(market.source_rate) })}</span>
            {#if linkFor("market_source")}<button type="button" class="profileLink" on:click={() => onOpenService(linkFor("market_source")!)}>{copy("Open {pair} on {venue}", { pair: market.source_pair, venue: venueName(market.venue) })} <span>↗</span></button>{:else if firstMarketUrl}<a href={firstMarketUrl} target="_blank" rel="noreferrer noopener" class="profileLink">{copy("Open {pair} on {venue}", { pair: market.source_pair, venue: venueName(market.venue) })} <span>↗</span></a>{/if}
          </div>
        </div></li>
        {#if route.bridge_currency}
          <li class="step" data-testid="instruction-step"><span class="stepNumber" aria-hidden="true">2</span><div class="stepBody">
            <h3>{copy("Convert {from} to {to}", { from: route.bridge_currency, to: route.target_currency ?? "" })}</h3>
            <p class="stepSummary">{copy("Do the second exchange on {venue} only after the first balance is available.", { venue: venueName(market.venue) })}</p>
            <ul class="checklist">
              <li>{copy("Open {pair} and check that it changes {from} into {to}.", { pair: market.target_pair, from: route.bridge_currency, to: route.target_currency ?? "" })}</li>
              <li>{copy("Check the current price, fee, and amount you should receive before pressing the exchange button.")}</li>
              <li>{copy("Before withdrawing, check the receiving asset and the network one more time.")}</li>
            </ul>
            <div class="counterparty"><div class="counterpartyTopline"><span class="counterpartyLabel">{copy("Spot market")}</span><span class="profileBadge">{venueName(market.venue)}</span></div><strong class="advertiser">{market.target_pair}</strong><span class="venueLine">{copy("Conversion rate {rate}", { rate: marketRate(market.target_rate) })}</span>
              {#if linkFor("market_target")}<button type="button" class="profileLink" on:click={() => onOpenService(linkFor("market_target")!)}>{copy("Open {pair} on {venue}", { pair: market.target_pair, venue: venueName(market.venue) })} <span>↗</span></button>{:else if secondMarketUrl}<a href={secondMarketUrl} target="_blank" rel="noreferrer noopener" class="profileLink">{copy("Open {pair} on {venue}", { pair: market.target_pair, venue: venueName(market.venue) })} <span>↗</span></a>{/if}
            </div>
          </div></li>
        {/if}
      {/if}
      {#if route.entry_offer_snapshot}
        <li class="step" data-testid="instruction-step"><span class="stepNumber" aria-hidden="true">{entryStepNumber}</span><div class="stepBody">
          <h3>{directFiat ? copy("Transfer {from} to {to} via {venue}", { from: route.source_currency, to: route.target_currency ?? "", venue: entryVenue }) : cryptoToCrypto ? copy("Sell {asset} for {amount}", { asset: route.source_currency, amount: route.bridge_currency ?? route.entry_asset }) : copy("Buy {asset} for {amount}", { asset: entryAsset, amount: money(route.source_amount_minor, route.source_currency) })}</h3>
          {#if entryDirect}
            <p class="stepSummary">{copy("Open the direct exchange on {venue}, check the final amount, and follow the provider's instructions.", { venue: entryVenue })}</p>
            <ul class="checklist">
              <li>{copy("Check the currencies, amount, current rate, fee, and limits before continuing.")}</li>
              <li>{copy("Sign in or complete verification on {venue}, if it asks you to, then follow the payment instructions shown there.", { venue: entryVenue })}</li>
              <li>{copy("After the exchange, check that the new balance is available before continuing.")}</li>
            </ul>
          {:else}
            <p class="stepSummary">{copy("Open the P2P listing on {venue}. Check the offer inside the platform before placing an order.", { venue: entryVenue })}</p>
            <ul class="checklist">
              <li>{copy("Before creating the order, compare the nickname and advertisement ID.")}</li>
              <li>{copy("Check the current rate, order limits, and payment method on {venue}.", { venue: entryVenue })}</li>
              {#if sourceFeeLabel}<li>{copy("This bank fee is only an estimate. Check the final bank fee before sending.")}</li>{/if}
              <li>{cryptoToCrypto ? copy("Release the asset only after you personally see that the payment has arrived.") : copy("Use only the payment details shown inside the order. After sending, mark the order as paid.")}</li>
            </ul>
          {/if}
          {#if entryGuide}
            <div class="providerGuide">
              <p>{entryGuide.description}</p>
              {#if entryGuideSteps.length}<ul class="checklist">{#each entryGuideSteps as step}<li>{step}</li>{/each}</ul>{/if}
              {#if entryGuide.links.length}<div class="guideLinks">{#each entryGuide.links as link}<a href={link.url} target="_blank" rel="noreferrer noopener" class="profileLink">{link.label} <span>↗</span></a>{/each}</div>{/if}
            </div>
          {/if}
          <AdvertiserCard offer={route.entry_offer_snapshot} label={entryDirect ? copy("Direct exchange on {venue}", { venue: entryVenue }) : `${cryptoToCrypto ? copy("Buyer") : copy("Seller")} ${copy("on {venue}", { venue: entryVenue })}`} serviceLink={linkFor("entry")} {venueNames} {onOpenService} />
        </div></li>
      {/if}
      {#if providerSwap}
        <li class="step" data-testid="instruction-step"><span class="stepNumber" aria-hidden="true">{providerStepNumber}</span><div class="stepBody">
          <h3>{copy("Swap {from} for {to} via {venue}", { from: providerSwapFrom ?? "", to: providerSwapTo ?? "", venue: venueName(route.route_provider) })}</h3>
          <p class="stepSummary">{route.entry_offer_snapshot ? copy("After you get {from}, send it to {venue} and exchange it for {to}.", { from: providerSwapFrom ?? "", venue: venueName(route.route_provider), to: providerSwapTo ?? "" }) : copy("Send {from} to {venue} first, then exchange it for {to}.", { from: providerSwapFrom ?? "", venue: venueName(route.route_provider), to: providerSwapTo ?? "" })}</p>
          {#if route.route_path?.length}<p class="routePath">{readablePath(route.route_path)}</p>{/if}
          {#if routeGuide}
            <div class="providerGuide">
              <p>{routeGuide.description}</p>
              {#if routeGuide.steps.length}<ul class="checklist">{#each routeGuide.steps as step}<li>{step}</li>{/each}</ul>{/if}
              {#if routeGuide.links.length}<div class="guideLinks">{#each routeGuide.links as link}<a href={link.url} target="_blank" rel="noreferrer noopener" class="profileLink">{link.label} <span>↗</span></a>{/each}</div>{/if}
            </div>
          {/if}
          <ul class="checklist">
            <li>{copy("Before sending, check the asset, the receiving asset, and the exact network.")}</li>
            <li>{copy("Check the current rate, provider fee, quote expiry, and any address, memo, or tag requirement.")}</li>
            <li>{copy("Wait until the new balance appears before considering this step finished.")}</li>
          </ul>
          {#if route.route_provider_url}<a href={route.route_provider_url} target="_blank" rel="noreferrer noopener" class="profileLink">{copy("Open {venue}", { venue: venueName(route.route_provider) })} <span>↗</span></a>{/if}
          <RouteExecutionPanel {route} />
        </div></li>
      {/if}
      {#if crossVenue}
        <li class="step" data-testid="instruction-step"><span class="stepNumber" aria-hidden="true">{transferStepNumber}</span><div class="stepBody">
          <h3>{copy("Transfer {asset} to {venue}", { asset: route.entry_asset, venue: exitVenue })}</h3>
          <p class="stepSummary">{copy("Send the purchased asset from {from} to the deposit address on {to} before opening the next order.", { from: entryVenue, to: exitVenue })}</p>
          <ul class="checklist">
            <li>{#if transferNetwork}{copy("Copy the deposit address from {venue}. Choose the exact {network} network on both platforms.", { venue: exitVenue, network: transferNetwork })}{:else}{copy("First check that both platforms support the same asset and network. Then copy the deposit address from {venue}.", { venue: exitVenue })}{/if}</li>
            <li>{copy("Check the complete address, memo or tag if required, and the withdrawal fee before confirming.")}</li>
            <li>{copy("Wait until {venue} shows the deposit as received before continuing.", { venue: exitVenue })}</li>
          </ul>
        </div></li>
      {/if}
      {#if route.exit_offer_snapshot}
        <li class="step" data-testid="instruction-step"><span class="stepNumber" aria-hidden="true">{exitStepNumber}</span><div class="stepBody">
          <h3>{cryptoToCrypto ? copy("Buy {asset} with {bridge}", { asset: route.target_currency ?? "", bridge: route.bridge_currency ?? route.entry_asset }) : copy("Sell {asset} for {amount}", { asset: route.entry_asset, amount: money(route.target_amount_minor, route.target_currency, route.target_amount) })}</h3>
          {#if exitDirect}
            <p class="stepSummary">{copy("Open the direct exchange on {venue}, check the final amount, and follow the provider's instructions.", { venue: exitVenue })}</p>
            <ul class="checklist">
              <li>{copy("Check the currencies, amount, current rate, fee, and limits before continuing.")}</li>
              <li>{copy("Sign in or complete verification on {venue}, if it asks you to, then follow the payment instructions shown there.", { venue: exitVenue })}</li>
              <li>{copy("After the sale, check that the money has arrived in your account before considering the exchange finished.")}</li>
            </ul>
          {:else}
            <p class="stepSummary">{copy("Open the P2P listing on {venue}. Check the offer inside the platform before placing an order.", { venue: exitVenue })}</p>
            <ul class="checklist">
              <li>{copy("Before creating the order, compare the nickname and advertisement ID.")}</li>
              <li>{copy("Check the current rate, order limits, {thing}, and expected amount.", { thing: cryptoToCrypto ? copy("asset network") : copy("recipient payment method") })}</li>
              {#if targetFeeLabel}<li>{copy("This bank fee is only an estimate. Check the final fee before accepting the payout.")}</li>{/if}
              <li>{cryptoToCrypto ? copy("Confirm the {asset} balance and network before withdrawing.", { asset: route.target_currency ?? "" }) : copy("Release the asset only after you personally see the payment in your bank or payment account.")}</li>
            </ul>
          {/if}
          {#if exitGuide}
            <div class="providerGuide">
              <p>{exitGuide.description}</p>
              {#if exitGuideSteps.length}<ul class="checklist">{#each exitGuideSteps as step}<li>{step}</li>{/each}</ul>{/if}
              {#if exitGuide.links.length}<div class="guideLinks">{#each exitGuide.links as link}<a href={link.url} target="_blank" rel="noreferrer noopener" class="profileLink">{link.label} <span>↗</span></a>{/each}</div>{/if}
            </div>
          {/if}
          <AdvertiserCard offer={route.exit_offer_snapshot} label={exitDirect ? copy("Direct exchange on {venue}", { venue: exitVenue }) : `${cryptoToCrypto ? copy("Seller") : copy("Buyer")} ${copy("on {venue}", { venue: exitVenue })}`} serviceLink={linkFor("exit")} {venueNames} {onOpenService} />
        </div></li>
      {/if}
    </ol>
    <div class="warning"><strong>{copy("Important")}</strong><span>{copy("Rates, limits, and offers can change. Check the provider, payment details, and network before sending money. Pay3Flow does not create orders or move money.")}</span>{#each route.warnings ?? [] as warning}<span>{warningText(warning)}</span>{/each}</div>
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

.providerGuide {
  margin-top: 14px;
  padding: 12px 13px;
  border: 1px solid rgba(109, 152, 0, 0.2);
  border-radius: 14px;
  background: rgba(181, 224, 58, 0.08);
}

.providerGuide > p {
  margin: 0;
  color: var(--color-text-soft);
  font-size: 11px;
  line-height: 1.5;
}

.providerGuide .checklist {
  margin-top: 10px;
}

.guideLinks {
  display: flex;
  flex-wrap: wrap;
  gap: 0 14px;
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

:global(html[data-theme="dark"]) .avatarVenue {
  border-color: #222222;
  background: #ffffff;
}

:global(html[data-theme="dark"]) .profileBadge {
  color: var(--color-accent);
}

</style>
