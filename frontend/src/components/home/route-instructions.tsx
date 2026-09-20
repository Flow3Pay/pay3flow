"use client";

import { useEffect, useState } from "react";

import { RouteCandidate } from "@/lib/exchange";
import { routeLocatorPath } from "@/lib/route-locator";

import styles from "./route-instructions.module.css";

const VENUE_NAMES: Record<string, string> = {
  binance: "Binance",
  bitget: "Bitget",
  bybit: "Bybit",
  okx: "OKX",
};

const venueName = (value: string | undefined) =>
  value ? VENUE_NAMES[value.toLowerCase()] ?? value : "P2P market";

const money = (minor: number | undefined, currency: string | undefined) =>
  minor == null
    ? "—"
    : `${(minor / 100).toLocaleString("en-US", { maximumFractionDigits: 2 })} ${currency ?? ""}`;

interface RouteInstructionsProps {
  route: RouteCandidate;
  onClose: () => void;
}

export function RouteInstructions({ route, onClose }: RouteInstructionsProps) {
  const [copied, setCopied] = useState(false);
  const locatorPath = routeLocatorPath(route);
  const entry = route.legs.find((leg) => leg.kind === "entry");
  const exit = route.legs.find((leg) => leg.kind === "exit");
  const entryVenue = venueName(entry?.provider);
  const exitVenue = venueName(exit?.provider);
  const crossVenue = entry?.provider !== exit?.provider;

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  const copyLocatorUrl = async () => {
    const locatorUrl = `${window.location.origin}${window.location.pathname}${window.location.search}${locatorPath}`;
    try {
      await navigator.clipboard.writeText(locatorUrl);
      setCopied(true);
    } catch {
      setCopied(false);
    }
  };

  return (
    <div className={styles.backdrop} role="presentation" onMouseDown={(event) => {
      if (event.target === event.currentTarget) onClose();
    }}>
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

        <div className={styles.shareRow}>
          <a href={locatorPath} target="_blank" rel="noreferrer noopener">
            Open route locator <span>↗</span>
          </a>
          <button type="button" className={styles.shareButton} onClick={copyLocatorUrl}>
            {copied ? "Link copied" : "Copy locator link"}
          </button>
        </div>

        <div className={styles.workflow}>
          <article className={styles.step}>
            <span className={styles.stepNumber}>01</span>
            <div>
              <strong>Buy {route.entry_asset} for {money(route.source_amount_minor, route.source_currency)}</strong>
              <p>Open the matched offer on {entryVenue}, check the advertiser and send the fiat payment using the selected bank.</p>
              {route.entry_offer_url && route.entry_offer_is_exact ? (
                <a href={route.entry_offer_url} target="_blank" rel="noreferrer noopener">
                  Open exact {entryVenue} offer <span>↗</span>
                </a>
              ) : (
                <span className={styles.missingLink}>
                  {entryVenue} does not expose a public deep-link for this ad. Ad ID: {route.entry_offer_ad_id ?? "—"}
                </span>
              )}
            </div>
          </article>

          {crossVenue && (
            <article className={styles.step}>
              <span className={styles.stepNumber}>02</span>
              <div>
                <strong>Transfer {route.entry_asset} to {exitVenue}</strong>
                <p>Send the asset to the second venue only after checking the exact network, address and transfer fee.</p>
              </div>
            </article>
          )}

          <article className={styles.step}>
            <span className={styles.stepNumber}>{crossVenue ? "03" : "02"}</span>
            <div>
              <strong>Sell {route.entry_asset} for {money(route.target_amount_minor, route.target_currency)}</strong>
              <p>Open the exit offer on {exitVenue}, sell the asset and choose the recipient payment method.</p>
              {route.exit_offer_url && route.exit_offer_is_exact ? (
                <a href={route.exit_offer_url} target="_blank" rel="noreferrer noopener">
                  Open exact {exitVenue} offer <span>↗</span>
                </a>
              ) : (
                <span className={styles.missingLink}>
                  {exitVenue} does not expose a public deep-link for this ad. Ad ID: {route.exit_offer_ad_id ?? "—"}
                </span>
              )}
            </div>
          </article>
        </div>

        <div className={styles.warning}>
          <strong>Important</strong>
          <span>Rates, limits and ads can change. Verify the offer, payment details and network on the exchange before sending money.</span>
        </div>
      </section>
    </div>
  );
}
