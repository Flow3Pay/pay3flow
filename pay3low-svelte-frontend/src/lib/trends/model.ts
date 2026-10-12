import type { RouteCandidate } from "../exchange";
import { routeWorkflowSteps } from "../workflow-steps.ts";

export type TrendMetric = "searches" | "appearances";
export type TrendPeriod = "1d" | "1w" | "1m";
export type TrendBlock = { asset: string; network: string; provider: string };
export type TrendObservation = TrendBlock & { appearances: number; source: string; target: string };
export type TrendEvent = { at: number; blocks: TrendObservation[] };
export type TrendItem = TrendBlock & {
  id: string;
  searches: number;
  appearances: number;
  history: number[];
  pairs: { source: string; target: string; count: number }[];
};

export const trendId = (block: TrendBlock) => JSON.stringify([block.asset.toUpperCase(), block.network.toLowerCase(), block.provider.toLowerCase()]);
const fiat = new Set(["RUB", "BYN", "AMD", "KZT", "UAH", "USD"]);
export const periodMs: Record<TrendPeriod, number> = { "1d": 86400000, "1w": 7 * 86400000, "1m": 30 * 86400000 };

/** Count a combination once per route, even when a cycle visits it twice. */
export function observeRoutes(routes: RouteCandidate[], at = Date.now()): TrendEvent {
  const observations = new Map<string, TrendObservation>();
  for (const route of routes) {
    if (route.status !== "complete") continue;
    const candidates: TrendBlock[] = routeWorkflowSteps(route).map((step) => ({
      asset: step.currency, network: step.network ?? "", provider: step.provider ?? "",
    }));
    const seen = new Set<string>();
    for (const candidate of candidates) {
      if ((!candidate.network && !candidate.provider) || !candidate.asset || fiat.has(candidate.asset.toUpperCase())) continue;
      const block = { ...candidate, asset: candidate.asset.toUpperCase(), network: candidate.network.toLowerCase(), provider: candidate.provider.toLowerCase() };
      const id = trendId(block);
      if (seen.has(id)) continue;
      seen.add(id);
      const key = JSON.stringify([id, route.source_currency, route.target_currency]);
      const previous = observations.get(key);
      if (previous) previous.appearances += 1;
      else observations.set(key, { ...block, appearances: 1, source: route.source_currency, target: route.target_currency ?? route.source_currency });
    }
  }
  return { at, blocks: [...observations.values()] };
}

export function aggregateTrends(events: TrendEvent[], period: TrendPeriod, metric: TrendMetric, now = Date.now()): TrendItem[] {
  const items = new Map<string, TrendItem>();
  const start = now - periodMs[period];
  for (const event of events) {
    if (event.at < start || event.at > now) continue;
    const searched = new Set<string>();
    for (const block of event.blocks) {
      const id = trendId(block);
      let item = items.get(id);
      if (!item) {
        item = { ...block, id, searches: 0, appearances: 0, history: Array(14).fill(0), pairs: [] };
        items.set(id, item);
      }
      const searchCount = searched.has(id) ? 0 : 1;
      searched.add(id);
      item.searches += searchCount;
      item.appearances += block.appearances;
      const bucket = Math.min(13, Math.floor((event.at - start) / periodMs[period] * 14));
      item.history[bucket] += metric === "searches" ? searchCount : block.appearances;
      const pair = item.pairs.find((pair) => pair.source === block.source && pair.target === block.target);
      if (pair) pair.count += block.appearances;
      else item.pairs.push({ source: block.source, target: block.target, count: block.appearances });
    }
  }
  return [...items.values()].map((item) => ({ ...item, pairs: item.pairs.sort((a, b) => b.count - a.count) }))
    .sort((a, b) => b[metric] - a[metric] || a.id.localeCompare(b.id));
}

/** Lay natural-width order blocks into staggered courses, highest scores first. */
export function layoutWall(items: TrendItem[], width: number, measured: Record<string, number> = {}) {
  const padding = 16;
  const right = Math.max(padding + 1, width - padding);
  const cells: { item: TrendItem; row: number; left: number; width: number; rank: number }[] = [];
  let index = 0;
  for (let row = 0; index < items.length; row += 1) {
    const ranges = row === 0 && width >= 700 ? [[width * .34, width * .58], [width * .79, right]]
      : row === 1 && width >= 700 ? [[width * .20, width * .64], [width * .70, right]]
      : row === 0 && width >= 260 ? [[width * .48, right]]
      : [[padding + (row % 2 ? Math.min(46, Math.max(0, width - 260)) : 0), right]];
    for (const [start, end] of ranges) {
      let left = start;
      while (index < items.length) {
        const item = items[index];
        const fallback = 58 + item.asset.length * 7 + (item.provider ? 35 + item.provider.length * 7 : 0);
        const brickWidth = Math.min(measured[item.id] ?? fallback, right - padding);
        if (left + brickWidth > end) break;
        cells.push({ item, row, left, width: brickWidth, rank: index + 1 });
        index += 1;
        left += brickWidth + 6;
      }
    }
    // A very narrow viewport still permits one block in a full-width course.
    if (row >= 2 && !cells.some((cell) => cell.row === row)) {
      const item = items[index];
      cells.push({ item, row, left: padding, width: right - padding, rank: index + 1 });
      index += 1;
    }
  }
  return cells;
}

export function trendSwapHref(item: TrendItem) {
  const pair = item.pairs[0];
  if (!pair) return "/#/swap";
  const params = new URLSearchParams({ assets: item.asset });
  if (item.provider) params.set("sources", item.provider);
  if (pair.source === item.asset && item.network) params.set("fromNetwork", item.network);
  if (pair.target === item.asset && item.network) params.set("toNetwork", item.network);
  return `/#/swap/${encodeURIComponent(pair.source)}/${encodeURIComponent(pair.target)}?${params}`;
}
