"use client";

import { RouteCandidate } from "@/lib/exchange";

import styles from "./side-panel.module.css";

const money = (minor: number | undefined, currency: string | undefined) =>
  minor == null ? "—" : `${(minor / 100).toLocaleString("en-US", { maximumFractionDigits: 2 })} ${currency ?? ""}`;

function spreadLabel(bps: number): string {
  const value = bps / 100;
  if (Math.abs(value) < 0.005) return "at cost";
  return `${value > 0 ? "+" : ""}${value.toFixed(2)}%`;
}

interface SidePanelProps {
  active: boolean;
  routes: RouteCandidate[];
  selectedQuoteId: string | null;
  onSelect: (route: RouteCandidate) => void;
}

export function SidePanel({ active, routes, selectedQuoteId, onSelect }: SidePanelProps) {
  return (
    <aside className={`${styles.side}${active ? ` ${styles.active}` : ""}`} aria-label="Found routes">
      <div className={styles.stage}>
        <div className={styles.quote}>
          <div className={styles.quoteInner}>
            {routes.length > 0 ? (
              <div
                className={styles.routeGroups}
                data-testid="route-groups"
                tabIndex={0}
                aria-label="Found routes"
              >
                <ul className={styles.routeList}>
                  {routes.map((route) => {
                    const complete = route.status === "complete";
                    const selected = route.quote_id === selectedQuoteId;
                    return (
                      <li key={route.route_id}>
                        <button
                          type="button"
                          className={`${route.is_current_best ? styles.routeBest : styles.routeRow}${selected ? ` ${styles.selected}` : ""}`}
                          disabled={!complete}
                          onClick={() => onSelect(route)}
                          data-testid={complete ? "complete-route" : "partial-route"}
                        >
                          <span className={styles.routeMain}>
                            <span className={styles.routeAmount}>
                              {complete
                                ? money(route.target_amount_minor, route.target_currency)
                                : "Preparing route"}
                            </span>
                            <span className={styles.routeMeta}>
                              {complete
                                ? `Fee ${money(route.fee_minor, route.source_currency)} · ${route.eta_minutes} min`
                                : "Checking recipient payout availability"}
                            </span>
                          </span>
                          <span className={styles.routeSide}>
                            {route.is_current_best && <span className={styles.routeBadgeBest}>BEST</span>}
                            {!route.is_current_best && complete && (
                              <span className={route.spread_bps < 0 ? styles.routeBadgeDelta : styles.routeBadgeFlat}>
                                {spreadLabel(route.spread_bps)}
                              </span>
                            )}
                            {!complete && <span className={styles.partialBadge}>SEARCHING</span>}
                            <span className={styles.routeName}>{complete ? "Select" : "Waiting"}</span>
                          </span>
                        </button>
                      </li>
                    );
                  })}
                </ul>
              </div>
            ) : (
              <div className={styles.emptyState}>Enter an amount and start route search.</div>
            )}
          </div>
        </div>
      </div>
    </aside>
  );
}
