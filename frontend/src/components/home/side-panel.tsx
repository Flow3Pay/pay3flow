"use client";

import { formatNumber, formatUsd, NETWORK_FEE_PERCENT, Token } from "./tokens";
import { Quote, QuoteCandidate } from "@/lib/rates";

import styles from "./side-panel.module.css";

/** Split a number into integer and fractional parts (en-US grouping). */
function splitAmount(value: number, maxFrac = 6): [string, string] {
  const abs = Math.abs(value);
  const int = Math.floor(abs);
  const frac = Math.round((abs - int) * 10 ** maxFrac);
  const fracStr = frac > 0 ? `.${frac.toString().padStart(maxFrac, "0").replace(/0+$/, "")}` : "";
  return [int.toLocaleString("en-US"), fracStr];
}

function formatDeviation(value: number): string {
  return `${value.toFixed(2)}%`;
}

interface SidePanelProps {
  active: boolean;
  sell: Token;
  buy: Token;
  rate: number;
  feeLabel: string;
  quote: Quote | null;
  /** Sell amount (in sell token units); used to size per-route outputs. */
  amount: number;
}

export function SidePanel({
  active,
  sell,
  buy,
  rate,
  feeLabel,
  quote,
  amount,
}: SidePanelProps) {
  const candidates = quote?.candidates ?? [];

  // Per-route economics: gross output before fees, then each acquirer's own
  // rate. Routes without a price fall back to the best route's fee, then to
  // the platform default, so every row stays comparable even when fmatch only
  // sends relevance ranks.
  const grossUsd = amount * sell.priceUsd;
  const grossBuy = amount * rate;
  const best = quote?.best ?? null;
  const bestPrice = best?.price ?? NETWORK_FEE_PERCENT / 100;
  const bestReceive = grossBuy * (1 - bestPrice);

  const routeRows = (c: QuoteCandidate) => {
    const price = c.price ?? bestPrice;
    const receive = grossBuy * (1 - price);
    const usdReceive = receive * buy.priceUsd;
    const feeUsd = grossUsd * price;
    const isBest = best != null && c.rank === best.rank;
    const deviation = isBest
      ? 0
      : bestReceive > 0
        ? ((bestReceive - receive) / bestReceive) * 100
        : 0;
    return { c, isBest, receive, usdReceive, feeUsd, deviation };
  };

  return (
    <div className={`${styles.side}${active ? ` ${styles.active}` : ""}`}>
      <div className={styles.stage}>
        <div className={styles.quote} aria-hidden={!active || undefined}>
          <div className={styles.quoteInner}>
            <div className={styles.quoteHead}>
              <span className={styles.quoteTitle}>Exchange summary</span>
            </div>

            <div className={styles.details}>
              <div className={styles.detailRow}>
                <span>Rate</span>
                <strong>
                  1 {sell.symbol} = {formatNumber(rate)} {buy.symbol}
                </strong>
              </div>
              <div className={styles.detailRow}>
                <span>Fee{quote?.best?.name ? ` · ${quote.best.name}` : ""}</span>
                <strong>{feeLabel}</strong>
              </div>
              {quote?.quotedAt && (
                <div className={styles.detailRow}>
                  <span>Quote</span>
                  <strong>{new Date(quote.quotedAt).toLocaleTimeString("en-US")}</strong>
                </div>
              )}
            </div>

            {candidates.length > 0 ? (
                <ul className={styles.routeList}>
                  {candidates.slice(0, 3).map((c) => {
                    const r = routeRows(c);
                    const isBest = r.isBest;
                    const showBadge = r.isBest || Math.abs(r.deviation) > 0.005;
                    return (
                      <li
                        key={`${c.shortId ?? c.name}-${c.rank}`}
                        className={isBest ? styles.routeBest : styles.routeRow}
                      >
                        <div className={styles.routeMain}>
                          <span className={styles.routeAmount}>
                            <span className={styles.routeAmountInt}>
                              {splitAmount(r.receive)[0]}
                            </span>
                            <span className={styles.routeAmountFrac}>
                              {splitAmount(r.receive)[1]}
                            </span>
                            <span className={styles.routeTicker}>{buy.symbol}</span>
                          </span>
                          <span className={styles.routeUsd}>≈ {formatUsd(r.usdReceive)}</span>
                          <span className={styles.routeFeeLine}>
                            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                              <path d="M14 13h2a2 2 0 0 1 2 2v2a2 2 0 0 0 4 0v-6.998a2 2 0 0 0-.59-1.42L18 5" />
                              <path d="M14 21V5a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v16" />
                              <path d="M2 21h13" />
                              <path d="M3 9h11" />
                            </svg>
                            {formatUsd(r.feeUsd)}
                          </span>
                        </div>
                        <div className={styles.routeSide}>
                          {showBadge ? (
                            isBest ? (
                              <span className={styles.routeBadgeBest}>
                                <svg width="13" height="13" viewBox="0 0 24 24" fill="#fff" aria-hidden="true">
                                  <path d="M12.1499 19.35C11.7399 19.35 11.3999 19.01 11.3999 18.6V16.5C11.3999 16.09 11.7399 15.75 12.1499 15.75C12.5599 15.75 12.8999 16.09 12.8999 16.5V18.6C12.8999 19.01 12.5599 19.35 12.1499 19.35Z" />
                                  <path d="M17.8999 22.75H6.3999V21C6.3999 19.48 7.6299 18.25 9.1499 18.25H15.1499C16.6699 18.25 17.8999 19.48 17.8999 21V22.75ZM7.8999 21.25H16.3999V21C16.3999 20.31 15.8399 19.75 15.1499 19.75H9.1499C8.4599 19.75 7.8999 20.31 7.8999 21V21.25Z" />
                                  <path d="M18.1499 22.75H6.1499C5.7399 22.75 5.3999 22.41 5.3999 22C5.3999 21.59 5.7399 21.25 6.1499 21.25H18.1499C18.5599 21.25 18.8999 21.59 18.8999 22C18.8999 22.41 18.5599 22.75 18.1499 22.75Z" />
                                  <path d="M18.43 12.4405C18.22 12.4405 18.01 12.3505 17.86 12.1805C17.67 11.9605 17.62 11.6505 17.74 11.3905C18.08 10.6105 18.25 9.78055 18.25 8.91055V5.91055C18.25 5.56055 18.19 5.22055 18.07 4.86055C18.06 4.83055 18.05 4.79055 18.04 4.75055C18.01 4.60055 18 4.45055 18 4.31055C18 3.90055 18.34 3.56055 18.75 3.56055H19.35C21.14 3.56055 22.6 5.06055 22.6 6.91055C22.6 8.44055 21.97 9.95055 20.88 11.0405C20.86 11.0605 20.8 11.1105 20.79 11.1205C20.2 11.6105 19.53 12.1605 18.63 12.4105C18.56 12.4305 18.5 12.4405 18.43 12.4405ZM19.68 5.09055C19.73 5.36055 19.75 5.64055 19.75 5.91055V8.91055C19.75 9.32055 19.72 9.71055 19.66 10.1105C19.72 10.0605 19.77 10.0205 19.83 9.97055C20.63 9.17055 21.1 8.05055 21.1 6.91055C21.1 6.01055 20.49 5.25055 19.68 5.09055Z" />
                                  <path d="M5.5799 12.3996C5.4999 12.3996 5.4299 12.3896 5.3499 12.3596C4.5299 12.0996 3.7599 11.6196 3.1199 10.9796C1.9699 9.70961 1.3999 8.31961 1.3999 6.84961C1.3999 5.02961 2.8299 3.59961 4.6499 3.59961H5.2999C5.5499 3.59961 5.7899 3.72961 5.9299 3.93961C6.0699 4.14961 6.0899 4.41961 5.9899 4.64961C5.8299 5.00961 5.7499 5.41961 5.7499 5.84961V8.84961C5.7499 9.70961 5.9199 10.5496 6.2699 11.3496C6.3899 11.6196 6.3299 11.9296 6.1399 12.1496C5.9899 12.3096 5.7899 12.3996 5.5799 12.3996ZM4.2999 5.12961C3.4899 5.28961 2.8999 5.98961 2.8999 6.84961C2.8999 7.93961 3.3399 8.98961 4.2099 9.94961C4.2499 9.99961 4.2999 10.0396 4.3499 10.0796C4.2799 9.66961 4.2499 9.25961 4.2499 8.84961V5.84961C4.2499 5.60961 4.2699 5.36961 4.2999 5.12961Z" />
                                  <path d="M12 16.75C7.73 16.75 4.25 13.27 4.25 9V6C4.25 3.38 6.38 1.25 9 1.25H15C17.62 1.25 19.75 3.38 19.75 6V9C19.75 13.27 16.27 16.75 12 16.75ZM9 2.75C7.21 2.75 5.75 4.21 5.75 6V9C5.75 12.45 8.55 15.25 12 15.25C15.45 15.25 18.25 12.45 18.25 9V6C18.25 4.21 16.79 2.75 15 2.75H9Z" />
                                </svg>
                                BEST
                              </span>
                            ) : (
                              <span className={styles.routeBadgeDelta}>
                                {formatDeviation(r.deviation)}
                              </span>
                            )
                          ) : (
                            <span className={styles.routeBadgeFlat}>-0.00%</span>
                          )}
                          <span className={styles.routeName}>{c.name}</span>
                        </div>
                      </li>
                    );
                  })}
                </ul>
              ) : (
                <div className={styles.routeEmpty}>
                  {quote ? "No acquirers matched that pair" : "waiting for a quote…"}
                </div>
              )}

            <div className={styles.quoteFooter}>
              <span className={styles.spinner} aria-hidden="true" />
              Pay3Flow picks the best route automatically
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}