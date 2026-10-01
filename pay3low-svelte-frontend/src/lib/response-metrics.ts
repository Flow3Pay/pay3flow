import type { P2pRouteSearchResponse, SourceStatus } from "$lib/exchange";

export interface SourceResponseTiming {
  last_response_ms: number | null;
  average_response_ms: number | null;
  sample_count: number;
  cache_hits: number;
}

type MutableTiming = SourceResponseTiming & { total_response_ms: number };

/**
 * Accumulates completed source responses for one route search.
 *
 * Route WebSocket messages are snapshots, so a source completion can appear
 * many times. The asset/leg/source identity makes those repeats idempotent.
 */
export class SearchResponseMetrics {
  private readonly seen = new Set<string>();
  private readonly timings = new Map<string, MutableTiming>();

  observe(response: P2pRouteSearchResponse): Map<string, SourceResponseTiming> {
    for (const asset of response.asset_statuses ?? []) {
      this.observeLeg(asset.asset, "entry", asset.entry_sources);
      this.observeLeg(asset.asset, "exit", asset.exit_sources);
    }
    return new Map(this.timings);
  }

  private observeLeg(asset: string, leg: "entry" | "exit", statuses: SourceStatus[]) {
    for (const status of statuses) {
      const source = status.source.toLowerCase();
      const key = `${asset}:${leg}:${source}`;
      if (this.seen.has(key)) continue;
      this.seen.add(key);

      const timing = this.timings.get(source) ?? {
        last_response_ms: null,
        average_response_ms: null,
        sample_count: 0,
        cache_hits: 0,
        total_response_ms: 0,
      };
      if (status.cached) {
        timing.cache_hits += 1;
      } else if (status.ok) {
        timing.last_response_ms = status.latency_ms;
        timing.total_response_ms += status.latency_ms;
        timing.sample_count += 1;
        timing.average_response_ms = Math.round(timing.total_response_ms / timing.sample_count);
      }
      this.timings.set(source, timing);
    }
  }
}
