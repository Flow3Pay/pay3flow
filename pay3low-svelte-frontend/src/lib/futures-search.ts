/*
Futures search draft paused by product decision; preserve this API client for
when the feature is resumed.
import { apiUrl } from "./api";

export interface FuturesOpportunity {
  asset: string;
  contract: string;
  long_venue: string;
  long_ask: number;
  long_url: string | null;
  short_venue: string;
  short_bid: number;
  short_url: string | null;
  spread_bps: number;
  estimated_gross_profit_usdt: number;
}

export interface FuturesSearchResponse {
  asset: string;
  notional_usdt: number;
  searched_at: string;
  opportunities: FuturesOpportunity[];
  sources: Array<{
    source: string;
    ok: boolean;
    latency_ms: number;
    offers_found: number;
    error: string | null;
  }>;
  warnings: string[];
}

export async function searchFutures(
  asset: string,
  notionalUsdt: number,
  sources: string[],
  signal: AbortSignal,
): Promise<FuturesSearchResponse> {
  const url = apiUrl("/api/p2p/futures");
  url.searchParams.set("asset", asset);
  url.searchParams.set("notional_usdt", String(notionalUsdt));
  if (sources.length) url.searchParams.set("sources", sources.join(","));
  const response = await fetch(url, { signal });
  if (!response.ok) throw new Error(await response.text());
  return response.json();
}
*/
