"use client";

import { RouteCandidate } from "@/lib/exchange";

import styles from "./side-panel.module.css";

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

const money = (minor: number | undefined, currency: string | undefined) =>
  minor == null
    ? "—"
    : `${(minor / 100).toLocaleString("en-US", { maximumFractionDigits: 2 })} ${currency ?? ""}`;

function spreadLabel(bps: number): string {
  const value = Math.abs(bps / 100);
  if (value < 0.005) return "Same output";
  return `${value.toFixed(2)}% less`;
}

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
  searching?: boolean;
  hasAmount?: boolean;
  sourceBank?: string;
  targetBank?: string;
}

function SearchSkeleton() {
  return (
    <div className={styles.skeletonList} aria-label="Searching live routes">
      {[0, 1, 2, 3].map((item) => (
        <div className={styles.skeletonCard} key={item} style={{ animationDelay: `${item * 80}ms` }}>
          <span className={styles.skeletonShort} />
          <span className={styles.skeletonLong} />
          <span className={styles.skeletonMedium} />
        </div>
      ))}
    </div>
  );
}

export function SidePanel({
  active,
  routes,
  selectedRouteId,
  onSelect,
  searching = false,
  hasAmount = false,
  sourceBank = "Sender bank",
  targetBank = "Recipient bank",
}: SidePanelProps) {
  const best = routes[0];

  return (
    <aside
      className={`${styles.side}${active ? ` ${styles.active}` : ""}`}
      aria-label="Found routes"
      aria-busy={searching}
      id="routes"
    >
      <div className={styles.panel}>
        <div className={styles.panelTop}>
          <div>
            <span className={styles.kicker}>Route intelligence</span>
            <strong>{routes.length ? "Live market paths" : "Awaiting your intent"}</strong>
          </div>
          <span className={searching ? styles.searchBadge : routes.length ? styles.liveBadge : styles.readyBadge}>
            <i />
            {searching ? "Scanning" : routes.length ? "Live" : "Ready"}
          </span>
        </div>

        {best && (
          <div className={styles.bestSummary}>
            <div>
              <span>Best recipient output</span>
              <strong>{money(best.target_amount_minor, best.target_currency)}</strong>
            </div>
            <div className={styles.summaryMeta}>
              <span>{routes.length}</span>
              <small>routes compared</small>
            </div>
          </div>
        )}

        <div className={styles.bankContext}>
          <span>{sourceBank}</span>
          <svg width="24" height="12" viewBox="0 0 24 12" fill="none" aria-hidden="true">
            <path d="M1 6h21m0 0-4-4m4 4-4 4" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
          <span>{targetBank}</span>
        </div>

        {searching ? (
          <SearchSkeleton />
        ) : routes.length > 0 ? (
          <div className={styles.routeGroups} data-testid="route-groups" tabIndex={0} aria-label="Found routes">
            <ul className={styles.routeList}>
              {routes.map((route, index) => {
                const complete = route.status === "complete";
                const selected = route.route_id === selectedRouteId;
                return (
                  <li key={route.route_id}>
                    <button
                      type="button"
                      className={`${styles.routeCard}${route.is_current_best ? ` ${styles.routeBest}` : ""}${selected ? ` ${styles.selected}` : ""}`}
                      disabled={!complete}
                      onClick={() => onSelect(route)}
                      data-testid={complete ? "complete-route" : "partial-route"}
                    >
                      <span className={styles.routeTopline}>
                        <span className={styles.routeRank}>#{String(index + 1).padStart(2, "0")}</span>
                        {route.is_current_best ? (
                          <span className={styles.bestBadge}>Best route</span>
                        ) : (
                          <span className={styles.deltaBadge}>{spreadLabel(route.spread_bps)}</span>
                        )}
                      </span>
                      <span className={styles.routeAmount}>{money(route.target_amount_minor, route.target_currency)}</span>
                      <span className={styles.workflow}>{workflowLabel(route)}</span>
                      <span className={styles.routeFooter}>
                        <span className={route.payment_methods_verified ? styles.verified : styles.unverified}>
                          <i />
                          {route.payment_methods_verified ? "Banks confirmed" : "Confirm bank support"}
                        </span>
                        <span className={styles.selectLabel}>{selected ? "Selected" : "Choose"} →</span>
                      </span>
                    </button>
                  </li>
                );
              })}
            </ul>
          </div>
        ) : (
          <div className={styles.emptyState}>
            <div className={styles.emptyVisual} aria-hidden="true">
              <span className={styles.emptyNode}>AM</span>
              <span className={styles.emptyPath}><i /><i /><i /></span>
              <span className={styles.emptyNode}>RU</span>
            </div>
            <div>
              <strong>{hasAmount ? "Preparing market scan" : "Your routes will appear here"}</strong>
              <p>
                {hasAmount
                  ? "Pay3Flow is ready to compare entry assets, venues and recipient payout options."
                  : "Enter an amount and we will assemble live cross-border paths in real time."}
              </p>
            </div>
            <div className={styles.emptyVenues}>
              <span>BINANCE</span><span>BYBIT</span><span>OKX</span><span>BITGET</span>
            </div>
          </div>
        )}
      </div>
    </aside>
  );
}
