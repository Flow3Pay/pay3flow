import type { RouteCandidate } from "../exchange";
import { observeRoutes, periodMs, type TrendEvent } from "./model";

export const TREND_STORAGE_KEY = "pay3flow.trends.activity.v1";
export const TREND_ACTIVITY_EVENT = "pay3flow:trends-activity";
const identifier = /^[a-z0-9._-]{1,100}$/i;

export function readTrendActivity(now = Date.now()): TrendEvent[] {
  if (typeof window === "undefined") return [];
  try {
    const data: unknown = JSON.parse(localStorage.getItem(TREND_STORAGE_KEY) ?? "[]");
    if (!Array.isArray(data)) return [];
    return data.filter((event): event is TrendEvent => event && Number.isFinite(event.at) && event.at > now - periodMs["1m"] && event.at <= now && Array.isArray(event.blocks) && event.blocks.length <= 500 && event.blocks.every((block: Record<string, unknown>) =>
      [block.asset, block.source, block.target].every((value) => typeof value === "string" && identifier.test(value)) &&
      typeof block.provider === "string" && (block.provider === "" || identifier.test(block.provider)) &&
      typeof block.network === "string" && (block.network === "" || identifier.test(block.network)) &&
      Number.isSafeInteger(block.appearances) && Number(block.appearances) > 0
    )).slice(-1000);
  } catch { return []; }
}

/** Store successful user searches only; refreshes and quote refinements are excluded. */
export function recordTrendSearch(routes: RouteCandidate[]) {
  if (typeof window === "undefined") return;
  const observation = observeRoutes(routes);
  if (!observation.blocks.length) return;
  try {
    localStorage.setItem(TREND_STORAGE_KEY, JSON.stringify([...readTrendActivity(), observation].slice(-1000)));
    window.dispatchEvent(new Event(TREND_ACTIVITY_EVENT));
  } catch { /* Storage restrictions must never interrupt route search. */ }
}
