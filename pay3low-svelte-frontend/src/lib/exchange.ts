import { apiUrl, wsUrl } from "./api";

export interface ExchangeCorridor {
  id: string;
  source_country: string;
  source_currency: string;
  target_country: string;
  target_currency: string;
  min_amount_minor: number | null;
  max_amount_minor: number | null;
  daily_limit_minor: number | null;
  metadata: Record<string, unknown>;
}

export interface CorridorsResponse {
  items: ExchangeCorridor[];
  terms_version: string;
}

export interface ProviderDefinition {
  id: string;
  slug: string;
  operation: "buy" | "sell";
  source_url: string;
  name: string;
  currencies: string[];
  banks: string[];
  searchable: boolean;
}

export interface ExchangeOrder {
  id: string;
  source_country: string;
  source_currency: string;
  source_amount_minor: number;
  source_method_type: string;
  target_country: string;
  target_currency: string;
  target_method_type: string;
  funding_status: string;
  status: string;
  selected_quote_id: string | null;
  failure_message: string | null;
  created_at: string;
}

export interface ExchangeQuote {
  id: string;
  order_id: string;
  target_amount_minor: number;
  target_currency: string;
  fee_minor: number;
  eta_minutes: number;
  rate: string;
  settlement_plan: {
    entry?: { to?: string; network?: string; provider?: string };
    exit?: { provider?: string };
  };
}

export interface RouteLeg {
  kind: "entry" | "exit";
  from: string;
  to: string;
  provider: string;
  status: "found" | "searching";
}

export interface RouteCandidate {
  route_id: string;
  quote_id?: string;
  status: "partial" | "complete";
  source_amount_minor: number;
  source_currency: string;
  source_method_icon_url?: string;
  entry_asset: string;
  entry_network: string;
  source_network?: string;
  target_network?: string;
  target_amount_minor?: number;
  target_currency?: string;
  target_method_icon_url?: string;
  route_kind?: "fiat_to_fiat" | "fiat_to_crypto" | "crypto_to_fiat" | "crypto_to_crypto";
  bridge_currency?: string | null;
  market_path?: CryptoMarketPath;
  spread_bps: number;
  fee_minor?: number;
  eta_minutes?: number;
  is_current_best?: boolean;
  is_live_market?: boolean;
  payment_methods_verified?: boolean;
  entry_offer_url?: string;
  entry_offer_is_exact?: boolean;
  entry_offer_ad_id?: string;
  exit_offer_url?: string;
  exit_offer_is_exact?: boolean;
  exit_offer_ad_id?: string;
  entry_offer_snapshot?: P2pOffer;
  exit_offer_snapshot?: P2pOffer;
  warnings?: string[];
  legs: RouteLeg[];
  services?: ServiceStats[];
  reputation?: CombinedReputation;
  service_links?: ServiceLink[];
}

export type ServiceVote = "like" | "dislike";

export interface ServiceStats {
  id: string;
  slug: string;
  display_name: string;
  executions_total: number;
  likes_total: number;
  dislikes_total: number;
  viewer_vote?: ServiceVote;
}

export interface CombinedReputation {
  executions_average: number;
  likes_average: number;
  dislikes_average: number;
}

export interface ServiceLink {
  service_id: string;
  service_slug: string;
  kind: "entry" | "exit" | "market_source" | "market_target";
  tracking_token: string;
}

export interface CryptoMarketPath {
  venue: string;
  source_pair: string;
  target_pair: string;
  source_rate: string;
  target_rate: string;
  intermediary_amount: string;
}

export interface P2pAdvertiser {
  id?: string | null;
  nickname: string;
  user_type?: string | null;
  is_merchant: boolean;
  is_verified: boolean;
  completed_orders_30d: number | null;
  completion_rate_30d: number | null;
}

export interface P2pOffer {
  source: string;
  ad_id: string;
  fiat: string;
  asset: string;
  price: string;
  available_asset: string;
  min_fiat: string;
  max_fiat: string;
  payment_methods: string[];
  pay_time_limit_minutes: number | null;
  advertiser: P2pAdvertiser;
  advertiser_profile_url?: string | null;
  source_url: string;
  source_url_is_exact?: boolean;
}

export interface P2pRoute {
  route_id?: string;
  rank: number;
  asset: string;
  entry_network?: string | null;
  source_network?: string | null;
  target_network?: string | null;
  source_fiat: string;
  source_amount: string;
  acquired_asset_amount: string;
  target_fiat: string;
  target_amount: string;
  effective_rate: string;
  same_venue: boolean;
  requires_asset_transfer: boolean;
  transfer_fee_included: boolean;
  route_kind?: "fiat_to_fiat" | "fiat_to_crypto" | "crypto_to_fiat" | "crypto_to_crypto";
  bridge_currency?: string | null;
  market_path?: CryptoMarketPath;
  payment_methods_verified: boolean;
  entry_offer?: P2pOffer;
  exit_offer?: P2pOffer;
  warnings: string[];
  services?: ServiceStats[];
  reputation?: CombinedReputation;
  service_links?: ServiceLink[];
}

export interface P2pRouteSearchResponse {
  search_id?: string;
  routes_found?: number;
  searched_at: string;
  source_fiat: string;
  target_fiat: string;
  source_amount: string;
  assets_searched: string[];
  can_exchange_to_target: boolean;
  routes: P2pRoute[];
}

export type P2pRouteStreamEvent =
  | { type: "search_started"; search_id: string; routes_found: number }
  | ({ type: "routes_updated" | "search_finished" } & P2pRouteSearchResponse)
  | { type: "search_failed"; search_id?: string; error: string };

export interface ServiceExecutionOpen {
  execution_id: string;
  newly_recorded: boolean;
  redirect_url: string;
  service: ServiceStats;
}

export type LiveRouteEvent =
  | ({ type: "entry_leg_found" } & RouteCandidate)
  | ({ type: "route_candidate_found" } & RouteCandidate)
  | { type: "best_route_updated"; route_id: string; quote_id: string }
  | { type: "order_status"; status: string }
  | { type: "search_started" }
  | { type: "exit_search_started"; route_id: string }
  | { type: "route_rejected"; route_id: string; reason: string }
  | { type: "search_finished"; best_quote_id: string | null }
  | { type: "search_failed"; error: string };

export interface FundingInstruction {
  id: string;
  method_type: string;
  amount_minor: number;
  currency: string;
  destination_ref: string;
  expires_at: string;
  raw_payload: { display_text?: string };
}

export interface ConfirmOrderResponse {
  order: ExchangeOrder;
  funding_instruction: FundingInstruction;
  settlement: {
    id: string;
    solver_id: string;
  };
}

async function request<T>(
  path: string,
  init: RequestInit = {},
  token?: string,
): Promise<T> {
  const headers = new Headers(init.headers);
  headers.set("Accept", "application/json");
  if (init.body) headers.set("Content-Type", "application/json");
  if (token) headers.set("Authorization", `Bearer ${token}`);
  const response = await fetch(apiUrl(path), { ...init, headers });
  if (!response.ok) {
    const payload = (await response.json().catch(() => null)) as
      | { error?: string; message?: string }
      | null;
    throw new Error(payload?.error ?? payload?.message ?? `Backend error (${response.status})`);
  }
  return (await response.json()) as T;
}

export async function authenticate(email: string, code: string): Promise<string> {
  const body = JSON.stringify({ email, code });
  const login = await fetch(apiUrl("/api/auth/login"), {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body,
  });
  if (login.ok) return ((await login.json()) as { token: string }).token;
  if (login.status !== 404) throw new Error("Sign-in failed. Check your email and code.");
  const registered = await request<{ token: string }>("/api/auth/register", {
    method: "POST",
    body,
  });
  return registered.token;
}

export function fetchCorridors(): Promise<CorridorsResponse> {
  return request("/api/exchange/corridors");
}

export function fetchProviders(): Promise<ProviderDefinition[]> {
  return request("/api/providers");
}

export function fetchP2pRoutes(query: {
  sourceFiat: string;
  targetFiat: string;
  sourceAmount: number;
  assets?: string[];
  intermediaryAssets?: string[];
  bridgeFiat?: string;
  sourceNetwork?: string;
  targetNetwork?: string;
  sourcePaymentMethod?: string;
  targetPaymentMethod?: string;
  sources?: string[];
  allowCrossVenue?: boolean;
  limit?: number;
  signal?: AbortSignal;
  anonymousId?: string;
}): Promise<P2pRouteSearchResponse> {
  const params = new URLSearchParams({
    source_fiat: query.sourceFiat,
    target_fiat: query.targetFiat,
    source_amount: String(query.sourceAmount),
    min_orders: "20",
    min_completion_rate: "0.9",
    allow_cross_venue: String(query.allowCrossVenue ?? true),
    limit: String(query.limit ?? 40),
  });
  if (query.assets?.length) {
    params.set("assets", query.assets.join(","));
  }
  if (query.intermediaryAssets?.length) {
    params.set("intermediary_assets", query.intermediaryAssets.join(","));
  }
  if (query.bridgeFiat) {
    params.set("bridge_fiat", query.bridgeFiat);
  }
  if (query.sourceNetwork) {
    params.set("source_network", query.sourceNetwork);
  }
  if (query.targetNetwork) {
    params.set("target_network", query.targetNetwork);
  }
  if (query.sourcePaymentMethod) {
    params.set("source_payment_method", query.sourcePaymentMethod);
  }
  if (query.targetPaymentMethod) {
    params.set("target_payment_method", query.targetPaymentMethod);
  }
  if (query.sources?.length) {
    params.set("sources", query.sources.join(","));
  }
  if (query.anonymousId) {
    params.set("anonymous_id", query.anonymousId);
  }
  return request(`/api/p2p/routes?${params.toString()}`, { signal: query.signal });
}

type P2pLiveQuery = Omit<Parameters<typeof fetchP2pRoutes>[0], "signal" | "anonymousId">;

function routeQueryPayload(query: P2pLiveQuery): Record<string, unknown> {
  return {
    source_fiat: query.sourceFiat,
    target_fiat: query.targetFiat,
    source_amount: query.sourceAmount,
    intermediary_assets: query.intermediaryAssets?.join(","),
    assets: query.assets?.join(","),
    bridge_fiat: query.bridgeFiat,
    source_network: query.sourceNetwork,
    target_network: query.targetNetwork,
    source_payment_method: query.sourcePaymentMethod,
    target_payment_method: query.targetPaymentMethod,
    sources: query.sources?.join(","),
    allow_cross_venue: query.allowCrossVenue ?? true,
    min_orders: 20,
    min_completion_rate: 0.9,
    limit: query.limit ?? 40,
  };
}

export function streamP2pRoutes(
  query: P2pLiveQuery,
  anonymousId: string,
  signal: AbortSignal,
  onEvent: (event: P2pRouteStreamEvent) => void,
): Promise<P2pRouteSearchResponse> {
  return new Promise((resolve, reject) => {
    const socket = new WebSocket(wsUrl("/ws/p2p/routes"));
    let settled = false;
    const fail = (error: Error) => {
      if (settled) return;
      settled = true;
      socket.close();
      reject(error);
    };
    const abort = () => fail(new DOMException("Route search cancelled", "AbortError"));
    signal.addEventListener("abort", abort, { once: true });
    socket.onopen = () => {
      socket.send(JSON.stringify({ anonymous_id: anonymousId, query: routeQueryPayload(query) }));
    };
    socket.onmessage = (message) => {
      try {
        const event = JSON.parse(message.data as string) as P2pRouteStreamEvent;
        onEvent(event);
        if (event.type === "search_failed") {
          fail(new Error(event.error));
        } else if (event.type === "search_finished") {
          settled = true;
          signal.removeEventListener("abort", abort);
          socket.close();
          resolve(event);
        }
      } catch {
        fail(new Error("Invalid live route response"));
      }
    };
    socket.onerror = () => fail(new Error("Live route stream unavailable"));
    socket.onclose = () => {
      if (!settled) fail(new Error("Live route stream closed before completion"));
    };
  });
}

export function recordServiceOpen(
  anonymousId: string,
  trackingToken: string,
): Promise<ServiceExecutionOpen> {
  return request("/api/service-executions/open", {
    method: "POST",
    body: JSON.stringify({ anonymous_id: anonymousId, tracking_token: trackingToken }),
  });
}

export function setServiceVote(
  serviceId: string,
  anonymousId: string,
  vote: ServiceVote | null,
): Promise<ServiceStats> {
  return request(`/api/services/${serviceId}/vote`, {
    method: "PUT",
    body: JSON.stringify({ anonymous_id: anonymousId, vote }),
  });
}

export function createOrder(
  token: string,
  body: Record<string, unknown>,
  idempotencyKey: string,
): Promise<ExchangeOrder> {
  return request(
    "/api/exchange/orders",
    {
      method: "POST",
      headers: { "Idempotency-Key": idempotencyKey },
      body: JSON.stringify(body),
    },
    token,
  );
}

export function fetchOrder(token: string, orderId: string): Promise<ExchangeOrder> {
  return request(`/api/exchange/orders/${orderId}`, {}, token);
}

export function fetchOrders(token: string): Promise<ExchangeOrder[]> {
  return request("/api/exchange/orders?limit=10", {}, token);
}

export function fetchQuotes(token: string, orderId: string): Promise<ExchangeQuote[]> {
  return request(`/api/exchange/orders/${orderId}/quotes`, {}, token);
}

export function confirmOrder(
  token: string,
  orderId: string,
  quoteId: string,
): Promise<ConfirmOrderResponse> {
  return request(
    `/api/exchange/orders/${orderId}/confirm`,
    { method: "POST", body: JSON.stringify({ quote_id: quoteId }) },
    token,
  );
}

export function confirmFunding(
  token: string,
  orderId: string,
  termsVersion: string,
): Promise<{ order: ExchangeOrder }> {
  return request(
    `/api/exchange/orders/${orderId}/funding/confirm`,
    {
      method: "POST",
      body: JSON.stringify({ accepts_terms: true, terms_version: termsVersion }),
    },
    token,
  );
}

export function submitMockProof(
  token: string,
  orderId: string,
  settlementId: string,
  solverId: string,
  amountMinor: number,
  currency: string,
): Promise<{ order: ExchangeOrder }> {
  return request(
    `/api/exchange/orders/${orderId}/proof`,
    {
      method: "POST",
      body: JSON.stringify({
        proof_type: "machine_receipt",
        proof_payload: {
          valid: true,
          settlement_id: settlementId,
          solver_id: solverId,
          amount_minor: amountMinor,
          currency,
          reference: `ui-mock-${orderId}`,
        },
      }),
    },
    token,
  );
}

export function openLiveRoutes(
  token: string,
  orderId: string,
  onEvent: (event: LiveRouteEvent) => void,
  onUnavailable: () => void,
): () => void {
  const url = new URL(`/api/exchange/orders/${orderId}/live`, wsUrl("/"));
  url.searchParams.set("access_token", token);
  const socket = new WebSocket(url);
  socket.onmessage = (message) => {
    try {
      onEvent(JSON.parse(message.data as string) as LiveRouteEvent);
    } catch {
      onUnavailable();
    }
  };
  socket.onerror = onUnavailable;
  return () => socket.close();
}

export function quoteToCandidate(quote: ExchangeQuote): RouteCandidate {
  const entry = quote.settlement_plan.entry;
  return {
    route_id: quote.id,
    quote_id: quote.id,
    status: "complete",
    source_amount_minor: 0,
    source_currency: "",
    entry_asset: entry?.to ?? "TOKEN",
    entry_network: entry?.network ?? "internal",
    target_amount_minor: quote.target_amount_minor,
    target_currency: quote.target_currency,
    spread_bps: 0,
    fee_minor: quote.fee_minor,
    eta_minutes: quote.eta_minutes,
    legs: [
      { kind: "entry", from: "", to: entry?.to ?? "TOKEN", provider: entry?.provider ?? "solver", status: "found" },
      { kind: "exit", from: entry?.to ?? "TOKEN", to: quote.target_currency, provider: quote.settlement_plan.exit?.provider ?? "solver", status: "found" },
    ],
  };
}
