import type { P2pRoute, P2pRouteSearchResponse, SourceStatus } from "./exchange";

export type DiscoveryMode = "exchange" | "arbitrage";
export interface DiscoveryQuery {
  mode: DiscoveryMode;
  source_fiat: string;
  target_fiat: string;
  source_amount: string;
  source_network?: string;
  target_network?: string;
  source_payment_method?: string;
  target_payment_method?: string;
  allow_cross_venue: boolean;
  max_actions: number;
  exchange_mode: "all" | "p2p" | "exchanger";
  sources?: string;
  intermediary_assets?: string;
  cursor?: string;
}
export interface SettlementEndpoint {
  currency: string;
  network: string | null;
  method_id: string | null;
  method: string | null;
  venue: string | null;
  verified: boolean;
}
export interface DiscoveryRoute extends P2pRoute {
  route_id: string;
  source_settlement: SettlementEndpoint;
  target_settlement: SettlementEndpoint;
  net_profit: string | null;
  net_profit_usd: number | null;
  estimated_profit_before_missing_costs_usd: number | null;
  return_percent: number | null;
  value_received_per_input: number | null;
  source_value_usd: number | null;
  target_value_usd: number | null;
  source_valuation: { price_usd: number; observed_at: string; source: string } | null;
  target_valuation: { price_usd: number; observed_at: string; source: string } | null;
  missing_costs: string[];
  profit_confirmed: boolean;
  observed_at: string;
  stale: boolean;
}
export interface DiscoveryResponse {
  searched_at: string;
  search_id: string;
  mode: DiscoveryMode;
  source_amount: string;
  reporting_currency: "USD";
  routes: DiscoveryRoute[];
  routes_found: number;
  routes_exhaustive: boolean;
  complete: boolean;
  coverage: { combinations_total: number; combinations_completed: number; combinations_failed: number; combinations_timed_out: number; batch_completed: number };
  provider_statuses: SourceStatus[];
  errors: string[];
  next_cursor: string | null;
}
export function routeExpired(route: DiscoveryRoute, now = Date.now()): boolean {
  const expires = route.quote_expires_at ? Date.parse(route.quote_expires_at) : Date.parse(route.observed_at) + 30_000;
  return !Number.isFinite(expires) || expires <= now;
}
export function mergeDiscoveryRoutes(previous: DiscoveryRoute[], incoming: DiscoveryRoute[], mode: DiscoveryMode, now = Date.now()): DiscoveryRoute[] {
  const rows = new Map(previous.filter(row => !routeExpired(row, now)).map(row => [row.route_id, row]));
  for (const row of incoming) if (!routeExpired(row, now)) rows.set(row.route_id, row);
  const category = (r: DiscoveryRoute) => mode === "arbitrage"
    ? r.net_profit_usd !== null && r.net_profit_usd > 0 ? 0 : r.net_profit_usd === null ? 1 : 2
    : r.value_received_per_input !== null && !r.missing_costs.length ? 0 : 1;
  const score = (r: DiscoveryRoute) => (mode === "arbitrage" ? r.net_profit_usd ?? r.estimated_profit_before_missing_costs_usd : r.value_received_per_input) ?? -Infinity;
  return [...rows.values()].sort((a,b) => category(a) - category(b) || score(b) - score(a) || (b.return_percent ?? -Infinity) - (a.return_percent ?? -Infinity) || a.route_id.localeCompare(b.route_id)).slice(0, 100);
}
export function discoveryLink(query: DiscoveryQuery): string {
  const params = new URLSearchParams();
  for (const [key,value] of Object.entries(query)) if (value !== undefined && key !== "cursor") params.set(key, String(value));
  return `/#/discover?${params}`;
}

/** Keep wildcard transport details outside Exchange's shared cards and actions. */
export function asExchangeResponse(query: DiscoveryQuery, response: DiscoveryResponse, routes: DiscoveryRoute[]): P2pRouteSearchResponse {
  return {
    search_id: response.search_id,
    searched_at: response.searched_at,
    source_fiat: query.source_fiat,
    target_fiat: query.target_fiat,
    source_amount: response.source_amount,
    assets_searched: [...new Set(routes.map(route => route.asset))],
    can_exchange_to_target: routes.length > 0,
    routes,
    routes_found: Math.max(response.routes_found, routes.length),
    routes_exhaustive: response.routes_exhaustive,
    provider_statuses: response.provider_statuses,
    stale: false,
  };
}
