"use client";

import { P2pOffer, RouteCandidate } from "@/lib/exchange";

import styles from "./route-locator.module.css";

const VENUE_NAMES: Record<string, string> = {
  binance: "Binance",
  bitget: "Bitget",
  bybit: "Bybit",
  okx: "OKX",
};

const venueName = (source: string) => VENUE_NAMES[source.toLowerCase()] ?? source;

const money = (minor: number | undefined, currency: string | undefined) =>
  minor == null ? "—" : `${(minor / 100).toLocaleString("en-US", { maximumFractionDigits: 2 })} ${currency ?? ""}`;

interface RouteLocatorProps {
  route: RouteCandidate;
}

function OfferCard({ offer, label }: { offer: P2pOffer | undefined; label: string }) {
  if (!offer) {
    return (
      <article className={`${styles.offer} ${styles.missing}`}>
        <span className={styles.offerLabel}>{label}</span>
        <strong>Offer snapshot unavailable</strong>
      </article>
    );
  }

  const exact = offer.source_url_is_exact;
  return (
    <article className={`${styles.offer} ${styles.highlighted}`} id={`locator-${offer.ad_id}`}>
      <div className={styles.offerTopline}>
        <span className={styles.offerLabel}>{label}</span>
        <span className={exact ? styles.exactBadge : styles.manualBadge}>
          {exact ? "Direct link" : "Manual check"}
        </span>
      </div>
      <strong className={styles.advertiser}>{offer.advertiser.nickname}</strong>
      <dl className={styles.details}>
        <div><dt>Venue</dt><dd>{venueName(offer.source)}</dd></div>
        <div><dt>Ad ID</dt><dd>{offer.ad_id}</dd></div>
        <div><dt>Rate</dt><dd>{offer.price} {offer.fiat} / {offer.asset}</dd></div>
        <div><dt>Limits</dt><dd>{offer.min_fiat}–{offer.max_fiat} {offer.fiat}</dd></div>
        <div><dt>Payment</dt><dd>{offer.payment_methods.length ? offer.payment_methods.join(", ") : "Check on venue"}</dd></div>
      </dl>
      {exact ? (
        <a href={offer.source_url} target="_blank" rel="noreferrer noopener" className={styles.offerLink}>
          Open exact {venueName(offer.source)} offer ↗
        </a>
      ) : (
        <a href={offer.source_url} target="_blank" rel="noreferrer noopener" className={styles.offerLink}>
          Open {venueName(offer.source)} market and verify this ad ↗
        </a>
      )}
    </article>
  );
}

export function RouteLocator({ route }: RouteLocatorProps) {
  const entry = route.entry_offer_snapshot;
  const exit = route.exit_offer_snapshot;
  const exact = Boolean(route.entry_offer_is_exact && route.exit_offer_is_exact);

  return (
    <section className={styles.page}>
      <div className={styles.eyebrow}>Pay3Flow route locator</div>
      <h1>Found route snapshot</h1>
      <p className={styles.intro}>
        This page preserves the exact offers selected by the search. Prices and availability can change, so verify both ads before sending money.
      </p>

      <div className={styles.summary}>
        <div><span>You send</span><strong>{money(route.source_amount_minor, route.source_currency)}</strong></div>
        <div className={styles.arrow}>→</div>
        <div><span>Recipient gets</span><strong>{money(route.target_amount_minor, route.target_currency)}</strong></div>
      </div>

      <div className={styles.status}>
        <i />
        {exact ? "Both offers have direct links" : "One or more offers require manual verification on the venue"}
      </div>

      <div className={styles.offers}>
        <OfferCard offer={entry} label={`01 · Buy ${route.entry_asset}`} />
        <div className={styles.connector}>↓</div>
        <OfferCard offer={exit} label={`02 · Sell ${route.entry_asset}`} />
      </div>

      <div className={styles.warning}>
        <strong>Before paying</strong>
        <span>Match the advertiser, price, limits, payment method and ad ID with the venue page. Pay3Flow never creates the P2P order or moves money for you.</span>
      </div>
    </section>
  );
}
