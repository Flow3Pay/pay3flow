import type { RouteCandidate } from "$lib/exchange";

export const OTC_PROVIDER = "pay3flow-otc";

/** Routes through our own desk are quoted from the local book, so they arrive before any remote venue. */
export function routeUsesOtc(route: Pick<RouteCandidate, "route_provider" | "legs" | "cycle_legs">): boolean {
  const hits = (value?: string | null) => value?.toLowerCase() === OTC_PROVIDER;
  return Boolean(hits(route.route_provider)
    || route.legs?.some(leg => hits(leg.provider))
    || route.cycle_legs?.some(leg => hits(leg.provider)));
}

/** The OTC desk quotes only this corridor, so a search mentions it only between those two. */
export function otcCorridor(source: string, target: string): boolean {
  const symbols = new Set(["EVER", "USDT"]);
  return symbols.has(source.toUpperCase()) && symbols.has(target.toUpperCase()) && source.toUpperCase() !== target.toUpperCase();
}
