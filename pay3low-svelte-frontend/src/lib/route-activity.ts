import { apiUrl } from "./api";

export const SEARCH_ACTIVITY_PERIODS = ["1h", "1d", "1w", "1m", "3m", "6m", "1y", "all"] as const;
export type SearchActivityPeriod = typeof SEARCH_ACTIVITY_PERIODS[number];
export const SEARCH_ACTIVITY_PERIOD_LABELS: Record<SearchActivityPeriod, string> = {
  "1h": "1 hour", "1d": "1 day", "1w": "1 week", "1m": "1 month",
  "3m": "3 months", "6m": "6 months", "1y": "1 year", all: "All time"
};

export interface RouteSearchActivityHour {
  started_at: string;
  count: number;
}

export interface RouteSearchActivity {
  source_currency: string;
  target_currency: string;
  hours: RouteSearchActivityHour[];
}

export async function fetchRouteSearchActivity(
  sourceCurrency: string,
  targetCurrency: string,
  period: SearchActivityPeriod,
  signal?: AbortSignal,
): Promise<RouteSearchActivity> {
  const url = apiUrl("/api/p2p/route-activity");
  url.searchParams.set("source_currency", sourceCurrency);
  url.searchParams.set("target_currency", targetCurrency);
  url.searchParams.set("period", period);
  const response = await fetch(url, { signal });
  if (!response.ok) throw new Error(`Route activity request failed: ${response.status}`);
  return response.json() as Promise<RouteSearchActivity>;
}
