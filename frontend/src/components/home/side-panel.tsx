"use client";

import { RouteCandidate } from "@/lib/exchange";

import styles from "./side-panel.module.css";

const money = (minor: number | undefined, currency: string | undefined) =>
  minor == null ? "—" : `${(minor / 100).toLocaleString("en-US", { maximumFractionDigits: 2 })} ${currency ?? ""}`;

function spreadLabel(bps: number): string {
  const value = bps / 100;
  if (Math.abs(value) < 0.005) return "same output";
  return value < 0 ? `${Math.abs(value).toFixed(2)}% lower` : `${value.toFixed(2)}% higher`;
}

const ASSET_NAMES: Record<string, string> = {
  BTC: "Bitcoin",
  ETH: "Ether",
  USDC: "USD Coin",
  USDT: "Tether",
};

const VENUE_NAMES: Record<string, string> = {
  binance: "Binance",
  bitget: "Bitget",
  bybit: "Bybit",
  okx: "OKX",
};

function venueName(value: string | undefined): string {
  if (!value) return "Searching";
  return VENUE_NAMES[value.toLowerCase()] ?? value;
}

function workflowLabel(route: RouteCandidate): string {
  const entry = route.legs.find((leg) => leg.kind === "entry");
  const exit = route.legs.find((leg) => leg.kind === "exit");
  const assetName = ASSET_NAMES[route.entry_asset.toUpperCase()];
  const asset = assetName ? `${route.entry_asset} ${assetName}` : route.entry_asset;
  return `${route.source_currency} → ${asset} (${venueName(entry?.provider)}) → ${route.target_currency ?? "—"} (${venueName(exit?.provider)})`;
}

interface SidePanelProps {
  active: boolean;
  routes: RouteCandidate[];
  selectedRouteId: string | null;
  onSelect: (route: RouteCandidate) => void;
}

export function SidePanel({ active, routes, selectedRouteId, onSelect }: SidePanelProps) {
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
                    const selected = route.route_id === selectedRouteId;
                    return (
                      <li key={route.route_id}>
                        <button
                          type="button"
                          className={`${route.is_current_best ? styles.routeBest : styles.routeRow}${selected ? ` ${styles.selected}` : ""}`}
                          disabled={!complete}
                          onClick={() => onSelect(route)}
                          data-testid={complete ? "complete-route" : "partial-route"}
                          title={
                            complete && route.is_live_market && !route.payment_methods_verified
                              ? "Confirm the selected banks on the venue before starting the transfer."
                              : undefined
                          }
                        >
                          <span className={styles.routeMain}>
                            <span className={styles.routeAmount}>
                              {complete
                                ? money(route.target_amount_minor, route.target_currency)
                                : "Preparing route"}
                            </span>
                            <span className={styles.routeMeta}>
                              {complete
                                ? route.is_live_market
                                  ? workflowLabel(route)
                                  : `Fee ${money(route.fee_minor, route.source_currency)} · ${route.eta_minutes} min`
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
