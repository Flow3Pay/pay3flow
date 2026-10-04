import { apiUrl } from "./api";

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
  anonymousId: string,
  signal?: AbortSignal,
): Promise<RouteSearchActivity> {
  const url = apiUrl("/api/p2p/route-activity");
  url.searchParams.set("source_currency", sourceCurrency);
  url.searchParams.set("target_currency", targetCurrency);
  if (anonymousId) url.searchParams.set("anonymous_id", anonymousId);
  const response = await fetch(url, { signal });
  if (!response.ok) throw new Error(`Route activity request failed: ${response.status}`);
  return response.json() as Promise<RouteSearchActivity>;
}
