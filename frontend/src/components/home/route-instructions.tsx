"use client";

import { useEffect } from "react";

import { P2pOffer, RouteCandidate } from "@/lib/exchange";

import styles from "./route-instructions.module.css";

const VENUE_NAMES: Record<string, string> = {
  binance: "Binance",
  bitget: "Bitget",
  bybit: "Bybit",
  okx: "OKX",
  rapira: "Rapira",
};

const VENUE_ICON_URLS: Record<string, string> = {
  binance: "https://www.google.com/s2/favicons?domain=binance.com&sz=64",
  bybit: "https://www.bybit.com/favicon.ico",
  okx: "https://www.okx.com/favicon.ico",
  bitget: "https://www.bitget.com/favicon.ico",
  rapira: "https://rapira.net/favicon.ico",
};

const VENUE_DOMAINS: Record<string, string> = {
  binance: "binance.com",
  bybit: "bybit.com",
  okx: "okx.com",
  bitget: "bitget.com",
  rapira: "rapira.net",
};

const SPOT_HOME_URLS: Record<string, string> = {
  binance: "https://www.binance.com/en/trade",
  bybit: "https://www.bybit.com/trade/spot/",
  okx: "https://www.okx.com/trade-spot/",
  bitget: "https://www.bitget.com/spot/",
};

const venueName = (value: string | undefined) =>
  value ? VENUE_NAMES[value.toLowerCase()] ?? value : "P2P market";

const money = (minor: number | undefined, currency: string | undefined) =>
  minor == null
    ? "—"
    : `${(minor / 100).toLocaleString("en-US", { maximumFractionDigits: 2 })} ${currency ?? ""}`;

const percentage = (value: number | null | undefined) =>
  value == null ? "—" : `${(value * 100).toFixed(1)}%`;

const marketRate = (value: string) => {
  const number = Number(value);
  return Number.isFinite(number)
    ? number.toLocaleString("en-US", { maximumFractionDigits: 12, useGrouping: false })
    : value;
};

function spotPair(
  symbol: string,
  firstAsset: string,
  secondAsset: string,
): { base: string; quote: string } | null {
  const normalizedSymbol = symbol.replace(/[^a-z0-9]/gi, "").toUpperCase();
  const first = firstAsset.toUpperCase();
  const second = secondAsset.toUpperCase();
  if (normalizedSymbol === `${first}${second}`) return { base: first, quote: second };
  if (normalizedSymbol === `${second}${first}`) return { base: second, quote: first };
  return null;
}

function spotMarketUrl(
  venue: string,
  symbol: string,
  firstAsset: string,
  secondAsset: string,
): string | null {
  const venueKey = venue.toLowerCase();
  const pair = spotPair(symbol, firstAsset, secondAsset);
  if (!pair) return SPOT_HOME_URLS[venueKey] ?? null;

  const { base, quote } = pair;
  switch (venueKey) {
    case "binance":
      return `https://www.binance.com/en/trade/${base}_${quote}?type=spot`;
    case "bybit":
      return `https://www.bybit.com/trade/spot/${base}/${quote}`;
    case "okx":
      return `https://www.okx.com/trade-spot/${base.toLowerCase()}-${quote.toLowerCase()}`;
    case "bitget":
      return `https://www.bitget.com/spot/${base}${quote}`;
    default:
      return null;
  }
}

function SpotMarketLink({
  venue,
  symbol,
  firstAsset,
  secondAsset,
}: {
  venue: string;
  symbol: string;
  firstAsset: string;
  secondAsset: string;
}) {
  const url = spotMarketUrl(venue, symbol, firstAsset, secondAsset);
  if (!url) return null;

  const pair = spotPair(symbol, firstAsset, secondAsset);
  const label = pair ? `${pair.base}/${pair.quote}` : symbol;
  return (
    <a href={url} target="_blank" rel="noreferrer noopener" className={styles.profileLink}>
      Open {label} on {venueName(venue)} <span>↗</span>
    </a>
  );
}

const avatarInitial = (nickname: string) => nickname.trim().charAt(0).toUpperCase() || "?";

function CounterpartyAvatar({ offer }: { offer: P2pOffer }) {
  const venueKey = offer.source.toLowerCase();
  const venueIcon = VENUE_ICON_URLS[venueKey];
  const venueDomain = VENUE_DOMAINS[venueKey];
  return (
    <span className={styles.counterpartyAvatar} aria-hidden="true">
      <span className={styles.avatarInitial}>{avatarInitial(offer.advertiser.nickname)}</span>
      {venueIcon && (
        <span className={styles.avatarVenue}>
          <img
            src={venueIcon}
            alt=""
            onError={(event) => {
              event.currentTarget.onerror = null;
              if (venueDomain) {
                event.currentTarget.src = `https://www.google.com/s2/favicons?domain=${venueDomain}&sz=64`;
              }
            }}
          />
        </span>
      )}
    </span>
  );
}

const fallbackProfileUrl = (offer: P2pOffer) => {
  if (offer.source.toLowerCase() !== "bybit" || !offer.advertiser.id) {
    return null;
  }
  return `https://www.bybit.com/en/p2p/profile/${encodeURIComponent(offer.advertiser.id)}/${encodeURIComponent(offer.asset)}/${encodeURIComponent(offer.fiat)}/item`;
};

interface RouteInstructionsProps {
  route: RouteCandidate;
  onClose: () => void;
}

function AdvertiserCard({
  offer,
  label,
}: {
  offer: P2pOffer | undefined;
  label: string;
}) {
  if (!offer) {
    return (
      <div className={`${styles.counterparty} ${styles.missing}`}>
        <span className={styles.counterpartyLabel}>{label}</span>
        <strong>Advertiser details unavailable</strong>
      </div>
    );
  }

  const venue = venueName(offer.source);
  const profileUrl = offer.advertiser_profile_url ?? fallbackProfileUrl(offer);
  const actionUrl = profileUrl ?? offer.source_url;
  const actionLabel = profileUrl
    ? `Open ${venue} profile`
    : `Open ${venue} P2P and find ${offer.advertiser.nickname}`;

  return (
    <div className={styles.counterparty}>
      <div className={styles.counterpartyIdentity}>
        <CounterpartyAvatar offer={offer} />
        <div className={styles.counterpartyIdentityCopy}>
          <div className={styles.counterpartyTopline}>
            <span className={styles.counterpartyLabel}>{label}</span>
            <span className={profileUrl ? styles.profileBadge : styles.manualBadge}>
              {profileUrl ? "User profile" : "Find by nickname"}
            </span>
          </div>
          <strong className={styles.advertiser}>{offer.advertiser.nickname}</strong>
          <span className={styles.venueLine}>
            {venue} · {offer.advertiser.is_merchant ? "Merchant" : "Advertiser"}
          </span>
        </div>
      </div>
      <div className={styles.metrics}>
        <span><b>{percentage(offer.advertiser.completion_rate_30d)}</b> completion</span>
        <span><b>{offer.advertiser.completed_orders_30d ?? "—"}</b> orders / 30d</span>
        <span><b>{offer.price} {offer.fiat}</b> rate</span>
      </div>
      <span className={styles.paymentLine}>
        Payment: {offer.payment_methods.length ? offer.payment_methods.join(", ") : "confirm on venue"}
      </span>
      <a href={actionUrl} target="_blank" rel="noreferrer noopener" className={styles.profileLink}>
        {actionLabel} <span>↗</span>
      </a>
      {!profileUrl && (
        <small className={styles.adHint}>Match the nickname and ad ID {offer.ad_id} before opening an order.</small>
      )}
    </div>
  );
}

export function RouteInstructions({ route, onClose }: RouteInstructionsProps) {
  const entry = route.legs.find((leg) => leg.kind === "entry");
  const exit = route.legs.find((leg) => leg.kind === "exit");
  const entryVenue = venueName(entry?.provider);
  const exitVenue = venueName(exit?.provider);
  const crossVenue = Boolean(entry && exit && entry.provider !== exit.provider);
  const cryptoToCrypto = route.route_kind === "crypto_to_crypto";
  const cryptoToFiat = route.route_kind === "crypto_to_fiat";
  const marketPath = route.market_path;

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  return (
    <div
      className={styles.backdrop}
      role="presentation"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <section className={styles.modal} role="dialog" aria-modal="true" aria-labelledby="route-instructions-title">
        <div className={styles.header}>
          <div>
            <span className={styles.eyebrow}>Selected route</span>
            <h2 id="route-instructions-title">How to complete this exchange</h2>
            <p>
              Estimated output: <strong>{money(route.target_amount_minor, route.target_currency)}</strong>
            </p>
          </div>
          <button type="button" className={styles.closeButton} onClick={onClose} aria-label="Close instructions">
            ×
          </button>
        </div>

        <div className={styles.workflow}>
          {cryptoToCrypto && marketPath && (
            <article className={styles.step}>
              <span className={styles.stepNumber}>01</span>
              <div>
                <strong>
                  {route.bridge_currency
                    ? `Swap ${route.source_currency} → ${route.bridge_currency} → ${route.target_currency}`
                    : `Swap ${route.source_currency} → ${route.target_currency}`}
                </strong>
                <p>
                  This route uses the {venueName(marketPath.venue)} exchange order book, not a P2P
                  advertiser, so there is no user profile. Open the spot pair below to place the trade.
                </p>
                <div className={styles.counterparty}>
                  <div className={styles.counterpartyTopline}>
                    <span className={styles.counterpartyLabel}>Spot market</span>
                    <span className={styles.profileBadge}>{venueName(marketPath.venue)}</span>
                  </div>
                  <strong className={styles.advertiser}>{marketPath.source_pair}</strong>
                  <span className={styles.venueLine}>Conversion rate {marketRate(marketPath.source_rate)}</span>
                  <SpotMarketLink
                    venue={marketPath.venue}
                    symbol={marketPath.source_pair}
                    firstAsset={route.source_currency}
                    secondAsset={route.bridge_currency ?? route.target_currency ?? route.entry_asset}
                  />
                  {route.bridge_currency && (
                    <>
                      <span className={styles.paymentLine}>
                        {marketPath.target_pair} · second leg rate {marketRate(marketPath.target_rate)}
                      </span>
                      <SpotMarketLink
                        venue={marketPath.venue}
                        symbol={marketPath.target_pair}
                        firstAsset={route.bridge_currency}
                        secondAsset={route.target_currency ?? route.entry_asset}
                      />
                    </>
                  )}
                </div>
              </div>
            </article>
          )}
          {route.entry_offer_snapshot && (
            <article className={styles.step}>
              <span className={styles.stepNumber}>01</span>
              <div>
                <strong>
                  {cryptoToCrypto
                    ? `Sell ${route.source_currency} for ${route.bridge_currency}`
                    : `Buy ${route.entry_asset} for ${money(route.source_amount_minor, route.source_currency)}`}
                </strong>
                <p>
                  {cryptoToCrypto
                    ? "Open the buyer's profile, verify the rate and limits, then complete the crypto sale on the venue."
                    : "Open the seller&apos;s profile, verify the rate and limits, then send the fiat payment using the selected bank."}
                </p>
                <AdvertiserCard
                  offer={route.entry_offer_snapshot}
                  label={`${cryptoToCrypto ? "Buyer" : "Seller"} on ${entryVenue}`}
                />
              </div>
            </article>
          )}

          {crossVenue && (
            <article className={styles.step}>
              <span className={styles.stepNumber}>02</span>
              <div>
                <strong>Transfer {route.entry_asset} to {exitVenue}</strong>
                <p>Send the asset to the second venue only after checking the exact network, address and transfer fee.</p>
              </div>
            </article>
          )}

          {route.exit_offer_snapshot && (
            <article className={styles.step}>
              <span className={styles.stepNumber}>{crossVenue ? "03" : route.entry_offer_snapshot ? "02" : "01"}</span>
              <div>
                <strong>
                  {cryptoToCrypto
                    ? `Buy ${route.target_currency} with ${route.bridge_currency}`
                    : cryptoToFiat
                      ? `Sell ${route.entry_asset} for ${money(route.target_amount_minor, route.target_currency)}`
                      : `Sell ${route.entry_asset} for ${money(route.target_amount_minor, route.target_currency)}`}
                </strong>
                <p>
                  {cryptoToCrypto
                    ? "Open the seller's profile, verify the network and limits, then buy the destination asset on the venue."
                    : "Open the buyer&apos;s profile, verify the recipient payment method and create the P2P order only on the venue."}
                </p>
                <AdvertiserCard
                  offer={route.exit_offer_snapshot}
                  label={`${cryptoToCrypto ? "Seller" : "Buyer"} on ${exitVenue}`}
                />
              </div>
            </article>
          )}
        </div>

        <div className={styles.warning}>
          <strong>Important</strong>
          <span>Rates, limits and ads can change. Confirm the user, payment details and network on the exchange before sending money. Pay3Flow never creates the order or moves funds.</span>
          {route.warnings?.map((warning) => <span key={warning}>{warning}</span>)}
        </div>
      </section>
    </div>
  );
}
