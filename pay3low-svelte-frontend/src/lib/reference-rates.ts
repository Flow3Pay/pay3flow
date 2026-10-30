import type { RouteCandidate } from "./exchange";

export type ReferenceMarket = "cash" | "noncash";
export interface ReferenceSource {
  name: string;
  url: string;
  published_at: string;
  fetched_at: string;
}
export interface ReferenceRate {
  from: string;
  to: string;
  rate: number;
  sources: ReferenceSource[];
  method: "identity" | "official" | "median_bank_midpoint" | "cross_via_usd";
  sample_count: number;
  stale: boolean;
}
export interface ReferenceRatesResponse {
  from: string;
  to: string;
  market: ReferenceMarket;
  rates: ReferenceRate[];
  unavailable_sources: Array<{ name: string; url: string }>;
}

/** Use major-unit quotes from the full result set, never the rendered page or selected route. */
export function routeReferenceRate(routes: RouteCandidate[], from: string, to: string, now: number) {
  const unique = new Map(routes.map(route => [route.route_id, route]));
  const samples: number[] = [];
  for (const route of unique.values()) {
    if (route.status !== "complete" || route.source_currency !== from || route.target_currency !== to) continue;
    if (route.quote_expires_at && !(Date.parse(route.quote_expires_at) > now)) continue;
    const source = Number(route.source_amount), target = Number(route.target_amount);
    if (!Number.isFinite(source) || !Number.isFinite(target) || source <= 0 || target <= 0) continue;
    const rate = target / source;
    if (Number.isFinite(rate) && rate > 0) samples.push(rate);
  }
  if (!samples.length) return null;
  samples.sort((a, b) => a - b);
  const middle = Math.floor(samples.length / 2);
  return {
    rate: samples.length % 2 ? samples[middle] : samples[middle - 1] / 2 + samples[middle] / 2,
    sample_count: samples.length,
  };
}

export function formatReferenceRate(rate: number, locale: string): string {
  return new Intl.NumberFormat(locale, { maximumSignificantDigits: 8 }).format(rate);
}
