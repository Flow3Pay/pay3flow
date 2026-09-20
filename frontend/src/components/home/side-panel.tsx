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
  rapira: "Rapira",
};

const ASSET_ICON_CDN = "https://cdn.jsdelivr.net/gh/spothq/cryptocurrency-icons@0.18.1/32/color";
const VENUE_ICON_URLS: Record<string, string> = {
  binance: "https://binance.com/favicon.ico",
  bybit: "https://www.bybit.com/favicon.ico",
  okx: "https://www.okx.com/favicon.ico",
  bitget: "https://www.bitget.com/favicon.ico",
  rapira: "https://rapira.net/favicon.ico",
};

const INSTRUCTION_ICON_URL = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAACkUlEQVR4AbTVXWjNcRzH8TNuRJQbyihlFLFSE7nhwqaUC4nWWmm78JAmCnm4QDGR0aTmYh5upKSFyEN5SEoolJILIpGUWBQhO17fMzvb6Zz/2Rm2Pu/z/f5+/+/D7//7//7/DUkN8t9/aZBOpw/ifKG1/nMDhecrXI9J/DSbo79qoNAoTIxKZWVltzCGvxpfzZ9isxpwAwWWyH6IE8hKkzsGW1EnZjeb0YAaSGyT1YHP2IscaXLYxBGsQEYlN1B8rYxGhNoUuxZOX8QsNa7GXWTUbwNJk3FadKzsNhtqMTcinB6MY/VnjcdqvpzNqGgDSQ2ibqAWayTG6s7xR+ISUmJm4TG/CR1iRrNZJTaQdEHUcXSiQuJRNsXGQ/7AnydmJ3sfU9HkWmwRt1cFG0jcJ2QxfqACUZBJxYqnc14jtMPPdcxUPLaQm6ucBgqX462QzdiOFgzDVUTxOEFP+DMQx3SXwgvw1Ligsg0UHi/iDT5irqRmbOO/wGzXH7GxPb/Yaa41IrbIMFnZBkJqEKqRmDlmik4x8RyhCfGDoZiDktS3QZyE77LWIbZjPXsPC3EALxHb84DdhJKUbWDV8fofkrXFym+y4cdRjBNy0bgKzYg3uFJMHFnD4so2iDBN4lsSH6s45w3G9XjmWjzV0+wZxHsQd1PSXeQ0kBznPIpWKXwyxlYaz2ERv8VcF+KTvN+42rVKtqjyGhSIjuP6yfwx9ChewJjr9y6KNrDCcSrWodXKv7EZ8eMFbDWoFVPOJqpoA1kb8RPxIWNyFHNdZjYgUYkNrCwe9EqZ7VYc28Ht1Z+5djOrxA5nCyqxgeh4oS6ze5Ck+M91xcWIZfKV2MAKO7EM7/PTumdce4eI+dI9k//7GwAA//+nN1TZAAAABklEQVQDANHZ3DHWgTEEAAAAAElFTkSuQmCC";

const FIAT_MARKS: Record<string, string> = {
  AMD: "🇦🇲",
  RUB: "🇷🇺",
  BYN: "🇧🇾",
};

const money = (minor: number | undefined, currency: string | undefined) =>
  minor == null
    ? "—"
    : `${(minor / 100).toLocaleString("en-US", { maximumFractionDigits: 2, useGrouping: false })} ${currency ?? ""}`;

function spreadLabel(bps: number): string {
  const value = Math.abs(bps / 100);
  if (value < 0.005) return "Same output";
  return `${value.toFixed(2)}% less`;
}

function venueName(value: string | undefined): string {
  if (!value) return "Searching";
  return VENUE_NAMES[value.toLowerCase()] ?? value;
}

function assetLabel(currency: string | undefined): string {
  if (!currency) return "—";
  const name = ASSET_NAMES[currency.toUpperCase()];
  return name ? `${currency} ${name}` : currency;
}

function assetIconUrl(currency: string): string | null {
  const symbol = currency.toUpperCase();
  return FIAT_MARKS[symbol] ? null : `${ASSET_ICON_CDN}/${symbol.toLowerCase()}.png`;
}

interface WorkflowStep {
  currency: string;
  provider?: string;
  iconUrl?: string;
}

function workflowSteps(route: RouteCandidate): WorkflowStep[] {
  const entry = route.legs.find((leg) => leg.kind === "entry");
  const exit = route.legs.find((leg) => leg.kind === "exit");
  const source = route.source_currency ?? "—";
  const target = route.target_currency ?? "—";

  if (!entry && exit) {
    return [
      { currency: source, provider: exit.provider, iconUrl: route.source_method_icon_url },
      { currency: target, iconUrl: route.target_method_icon_url },
    ];
  }
  if (entry && !exit) {
    return [
      { currency: source, iconUrl: route.source_method_icon_url },
      { currency: target, provider: entry.provider, iconUrl: route.target_method_icon_url },
    ];
  }
  if (route.bridge_currency) {
    return [
      { currency: source, provider: entry?.provider, iconUrl: route.source_method_icon_url },
      { currency: route.bridge_currency },
      { currency: target, provider: exit?.provider, iconUrl: route.target_method_icon_url },
    ];
  }
  return [
    { currency: source, iconUrl: route.source_method_icon_url },
    { currency: route.entry_asset ?? "—", provider: entry?.provider },
    { currency: target, provider: exit?.provider, iconUrl: route.target_method_icon_url },
  ];
}

function WorkflowIcon({ currency, iconUrl }: { currency: string; iconUrl?: string }) {
  const mark = FIAT_MARKS[currency.toUpperCase()];
  if (mark && !iconUrl) return <span className={styles.workflowFlag} aria-hidden="true">{mark}</span>;

  return (
    <span className={styles.workflowIcon} aria-hidden="true">
      <img src={iconUrl ?? assetIconUrl(currency) ?? `${ASSET_ICON_CDN}/generic.png`} alt="" />
    </span>
  );
}

function WorkflowVenue({ provider }: { provider?: string }) {
  if (!provider) return null;
  const iconUrl = VENUE_ICON_URLS[provider.toLowerCase()];
  return (
    <span className={styles.workflowVenue}>
      <span aria-hidden="true">(</span>
      {iconUrl && (
        <span className={styles.workflowVenueIcon} aria-hidden="true">
          <img
            src={iconUrl}
            alt=""
            onError={(event) => {
              event.currentTarget.onerror = null;
              event.currentTarget.src = `https://www.google.com/s2/favicons?domain=${provider.toLowerCase()}.com&sz=64`;
            }}
          />
        </span>
      )}
      <span>{venueName(provider)}</span>
      <span aria-hidden="true">)</span>
    </span>
  );
}

function workflowLabel(route: RouteCandidate): string {
  return workflowSteps(route)
    .map((step) => `${assetLabel(step.currency)}${step.provider ? ` (${venueName(step.provider)})` : ""}`)
    .join(" → ");
}

interface SidePanelProps {
  active: boolean;
  routes: RouteCandidate[];
  selectedRouteId: string | null;
  onSelect: (route: RouteCandidate) => void;
  onOpenInstructions: (route: RouteCandidate) => void;
  searching?: boolean;
  searched?: boolean;
  hasAmount?: boolean;
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
  onOpenInstructions,
  searching = false,
  searched = false,
  hasAmount = false,
}: SidePanelProps) {
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
            <strong>{routes.length ? "Live market paths" : "Awaiting your intent"}</strong>
          </div>
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
                    <div className={styles.routeCardShell}>
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
                        <span className={styles.workflow} aria-label={workflowLabel(route)}>
                          {workflowSteps(route).map((step, stepIndex) => (
                            <span className={styles.workflowPart} key={`${step.currency}-${stepIndex}`}>
                              {stepIndex > 0 && <span className={styles.workflowArrow} aria-hidden="true">→</span>}
                              <span className={styles.workflowAsset}>
                                <WorkflowIcon currency={step.currency} iconUrl={step.iconUrl} />
                                <span>{assetLabel(step.currency)}</span>
                              </span>
                              <WorkflowVenue provider={step.provider} />
                            </span>
                          ))}
                        </span>
                      </button>
                      {complete && (
                        <button
                          type="button"
                          className={styles.instructionButton}
                          onClick={() => {
                            onSelect(route);
                            onOpenInstructions(route);
                          }}
                          data-testid="route-instructions-button"
                        >
                          <img src={INSTRUCTION_ICON_URL} alt="icon" />
                        </button>
                      )}
                    </div>
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
              <strong>
                {searched && hasAmount
                  ? "No routes found"
                  : hasAmount
                    ? "Preparing market scan"
                    : "Your routes will appear here"}
              </strong>
              <p>
                {searched && hasAmount
                  ? "No compatible live offers were found for this amount and payment method."
                  : hasAmount
                    ? "Pay3Flow is ready to compare entry assets, venues and recipient payout options."
                    : "Enter an amount and we will assemble live cross-border paths in real time."}
              </p>
            </div>
            <div className={styles.emptyVenues}>
              <span>BINANCE</span><span>BYBIT</span><span>OKX</span><span>BITGET</span><span>RAPIRA</span>
            </div>
          </div>
        )}
      </div>
    </aside>
  );
}
