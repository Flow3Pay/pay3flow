import assert from "node:assert/strict";
import test from "node:test";

import { SearchResponseMetrics } from "../src/lib/response-metrics.ts";

const status = (source: string, latency_ms: number, cached = false) => ({
  source,
  ok: true,
  cached,
  latency_ms,
  offers_found: 1,
  error: null,
});

const snapshot = (entry_sources: ReturnType<typeof status>[], exit_sources: ReturnType<typeof status>[]) => ({
  searched_at: "2026-10-01T00:00:00Z",
  source_fiat: "AMD",
  target_fiat: "RUB",
  source_amount: "100",
  assets_searched: ["USDT"],
  can_exchange_to_target: true,
  routes: [],
  asset_statuses: [{
    asset: "USDT",
    entry_offers: 0,
    exit_offers: 0,
    routes_built: 0,
    can_exchange_to_target: true,
    entry_sources,
    exit_sources,
  }],
});

test("response metrics count each completed source sample once per search", () => {
  const metrics = new SearchResponseMetrics();
  metrics.observe(snapshot([status("fast", 10)], []));
  metrics.observe(snapshot([status("fast", 10)], [status("fast", 30)]));
  metrics.observe(snapshot([status("fast", 10)], [status("fast", 30)]));

  assert.deepEqual(metrics.observe(snapshot([status("fast", 10)], [status("fast", 30)])).get("fast"), {
    last_response_ms: 30,
    average_response_ms: 20,
    sample_count: 2,
    cache_hits: 0,
    total_response_ms: 40,
  });
});

test("cache observations are explicit and never lower the network average", () => {
  const metrics = new SearchResponseMetrics();
  metrics.observe(snapshot([status("bybit", 120)], []));
  const timing = metrics.observe(snapshot([status("bybit", 120)], [status("bybit", 0, true)])).get("bybit");

  assert.equal(timing?.last_response_ms, 120);
  assert.equal(timing?.average_response_ms, 120);
  assert.equal(timing?.sample_count, 1);
  assert.equal(timing?.cache_hits, 1);
});

test("provider completion is counted once across repeated route snapshots", () => {
  const metrics = new SearchResponseMetrics();
  const response = {
    ...snapshot([], []),
    provider_statuses: [status("id-pay", 18)],
  };

  metrics.observe(response);
  const timing = metrics.observe(response).get("id-pay");

  assert.equal(timing?.last_response_ms, 18);
  assert.equal(timing?.average_response_ms, 18);
  assert.equal(timing?.sample_count, 1);
});
