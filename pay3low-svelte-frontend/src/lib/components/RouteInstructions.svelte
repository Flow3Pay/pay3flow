<script lang="ts">
  import { onMount } from "svelte";
  import type { RouteCandidate } from "$lib/exchange";
  import AdvertiserCard from "./AdvertiserCard.svelte";

  export let route: RouteCandidate;
  export let onClose: () => void;
  const VENUE_NAMES: Record<string, string> = { binance: "Binance", bitget: "Bitget", bybit: "Bybit", okx: "OKX", rapira: "Rapira" };
  const venueName = (value?: string) => value ? VENUE_NAMES[value.toLowerCase()] ?? value : "P2P market";
  const money = (minor?: number, currency?: string) => minor == null ? "—" : `${(minor / 100).toLocaleString("en-US", { maximumFractionDigits: 2 })} ${currency ?? ""}`;
  const marketRate = (value: string) => Number.isFinite(Number(value)) ? Number(value).toLocaleString("en-US", { maximumFractionDigits: 12, useGrouping: false }) : value;

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
  onMount(() => {
    const handler = (event: KeyboardEvent) => event.key === "Escape" && onClose();
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  });
  $: entry = route.legs.find((leg) => leg.kind === "entry");
  $: exit = route.legs.find((leg) => leg.kind === "exit");
  $: entryVenue = venueName(entry?.provider);
  $: exitVenue = venueName(exit?.provider);
  $: crossVenue = Boolean(entry && exit && entry.provider !== exit.provider);
  $: cryptoToCrypto = route.route_kind === "crypto_to_crypto";
  $: cryptoToFiat = route.route_kind === "crypto_to_fiat";
  $: firstMarketUrl = route.market_path ? spotUrl(route.market_path.venue, route.market_path.source_pair, route.source_currency, route.bridge_currency ?? route.target_currency ?? route.entry_asset) : null;
  $: secondMarketUrl = route.market_path && route.bridge_currency ? spotUrl(route.market_path.venue, route.market_path.target_pair, route.bridge_currency, route.target_currency ?? route.entry_asset) : null;
</script>

<div class="backdrop" role="presentation" on:mousedown={backdrop}>
  <div class="modal" role="dialog" aria-modal="true" aria-labelledby="route-instructions-title" tabindex="-1">
    <div class="header"><div><span class="eyebrow">Selected route</span><h2 id="route-instructions-title">How to complete this exchange</h2><p>Estimated output: <strong>{money(route.target_amount_minor, route.target_currency)}</strong></p></div><button type="button" class="closeButton" on:click={onClose} aria-label="Close instructions">×</button></div>
    <div class="workflow">
      {#if cryptoToCrypto && route.market_path}
        {@const market = route.market_path}
        <article class="step"><span class="stepNumber">01</span><div>
          <strong>{route.bridge_currency ? `Swap ${route.source_currency} → ${route.bridge_currency} → ${route.target_currency}` : `Swap ${route.source_currency} → ${route.target_currency}`}</strong>
          <p>This route uses the {venueName(market.venue)} exchange order book, not a P2P advertiser, so there is no user profile. Open the spot pair below to place the trade.</p>
          <div class="counterparty"><div class="counterpartyTopline"><span class="counterpartyLabel">Spot market</span><span class="profileBadge">{venueName(market.venue)}</span></div><strong class="advertiser">{market.source_pair}</strong><span class="venueLine">Conversion rate {marketRate(market.source_rate)}</span>
            {#if firstMarketUrl}<a href={firstMarketUrl} target="_blank" rel="noreferrer noopener" class="profileLink">Open {market.source_pair} on {venueName(market.venue)} <span>↗</span></a>{/if}
            {#if route.bridge_currency}<span class="paymentLine">{market.target_pair} · second leg rate {marketRate(market.target_rate)}</span>{#if secondMarketUrl}<a href={secondMarketUrl} target="_blank" rel="noreferrer noopener" class="profileLink">Open {market.target_pair} on {venueName(market.venue)} <span>↗</span></a>{/if}{/if}
          </div>
        </div></article>
      {/if}
      {#if route.entry_offer_snapshot}
        <article class="step"><span class="stepNumber">01</span><div><strong>{cryptoToCrypto ? `Sell ${route.source_currency} for ${route.bridge_currency}` : `Buy ${route.entry_asset} for ${money(route.source_amount_minor, route.source_currency)}`}</strong><p>{cryptoToCrypto ? "Open the buyer's profile, verify the rate and limits, then complete the crypto sale on the venue." : "Open the seller's profile, verify the rate and limits, then send the fiat payment using the selected bank."}</p><AdvertiserCard offer={route.entry_offer_snapshot} label={`${cryptoToCrypto ? "Buyer" : "Seller"} on ${entryVenue}`} /></div></article>
      {/if}
      {#if crossVenue}<article class="step"><span class="stepNumber">02</span><div><strong>Transfer {route.entry_asset} to {exitVenue}</strong><p>Send the asset to the second venue only after checking the exact network, address and transfer fee.</p></div></article>{/if}
      {#if route.exit_offer_snapshot}
        <article class="step"><span class="stepNumber">{crossVenue ? "03" : route.entry_offer_snapshot ? "02" : "01"}</span><div><strong>{cryptoToCrypto ? `Buy ${route.target_currency} with ${route.bridge_currency}` : cryptoToFiat ? `Sell ${route.entry_asset} for ${money(route.target_amount_minor, route.target_currency)}` : `Sell ${route.entry_asset} for ${money(route.target_amount_minor, route.target_currency)}`}</strong><p>{cryptoToCrypto ? "Open the seller's profile, verify the network and limits, then buy the destination asset on the venue." : "Open the buyer's profile, verify the recipient payment method and create the P2P order only on the venue."}</p><AdvertiserCard offer={route.exit_offer_snapshot} label={`${cryptoToCrypto ? "Seller" : "Buyer"} on ${exitVenue}`} /></div></article>
      {/if}
    </div>
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
}

.header p strong {
  color: var(--color-text);
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
  display: flex;
  flex-direction: column;
  gap: 12px;
  margin-top: 25px;
}

.step {
  display: grid;
  grid-template-columns: 34px 1fr;
  gap: 13px;
  padding: 15px;
  border: 1px solid var(--color-border);
  border-radius: 17px;
  background: rgba(255, 255, 255, 0.72);
}

.stepNumber {
  display: grid;
  width: 30px;
  height: 30px;
  place-items: center;
  border-radius: 10px;
  background: var(--color-primary);
  color: var(--color-accent);
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 800;
}

.step strong {
  display: block;
  font-size: 13px;
  line-height: 1.35;
}

.step p {
  margin: 6px 0 10px;
  color: var(--color-text-soft);
  font-size: 11px;
  line-height: 1.5;
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

.step {
  border-radius: 10px;
  background: #fbfcfa;
  transition: border-color 0.14s ease, background 0.14s ease, transform 0.14s ease;
}

.step:hover {
  border-color: #b7cead;
  background: #fff;
  transform: translateX(2px);
}

.counterparty {
  border-color: #d4e4c5;
  background: #f4f8f1;
}

.stepNumber {
  border-radius: 8px;
  background: var(--color-primary);
  color: var(--color-accent);
}

@media (max-width: 560px) {
  .backdrop {
    padding: 12px;
  }

  .modal {
    max-height: calc(100vh - 24px);
    padding: 22px 18px;
    border-radius: 23px;
  }

  .header h2 {
    font-size: 23px;
  }
}

:global(html[data-theme="dark"]) .modal {
  border-color: var(--color-border-strong);
  background: rgba(25, 25, 25, 0.99);
  box-shadow: var(--shadow-pop);
}

:global(html[data-theme="dark"]) .closeButton,
:global(html[data-theme="dark"]) .step,
:global(html[data-theme="dark"]) .counterparty {
  border-color: #3b3b3b;
  background: #222222;
}

:global(html[data-theme="dark"]) .step:hover {
  border-color: #555555;
  background: #2a2a2a;
}

:global(html[data-theme="dark"]) .avatarVenue {
  border-color: #222222;
  background: #ffffff;
}

:global(html[data-theme="dark"]) .profileBadge {
  color: var(--color-accent);
}

</style>
