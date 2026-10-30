import assert from "node:assert/strict";
import test from "node:test";
import { routeReferenceRate, formatReferenceRate } from "../src/lib/reference-rates.ts";
import type { RouteCandidate } from "../src/lib/exchange.ts";

const now = Date.parse("2026-10-30T12:00:00Z");
const route = (id: string, rate: number, overrides: Partial<RouteCandidate> = {}) => ({
  route_id: id, status: "complete", source_currency: "BTC", target_currency: "USD",
  source_amount: "2", target_amount: String(2 * rate), ...overrides,
}) as RouteCandidate;

test("route median handles odd/even counts and uses major units", () => {
  assert.deepEqual(routeReferenceRate([route("a", 90), route("b", 10), route("c", 20)], "BTC", "USD", now), { rate: 20, sample_count: 3 });
  assert.deepEqual(routeReferenceRate([route("a", 90), route("b", 10)], "BTC", "USD", now), { rate: 50, sample_count: 2 });
  assert.deepEqual(routeReferenceRate([route("a", 0.000000005)], "BTC", "USD", now), { rate: 0.000000005, sample_count: 1 });
});

test("rejects partial, expired, mismatched and invalid quotes without minor-unit fallback", () => {
  const invalid = [
    route("partial", 20, { status: "partial" }), route("expired", 20, { quote_expires_at: "2026-10-30T11:00:00Z" }),
    route("boundary", 20, { quote_expires_at: "2026-10-30T12:00:00Z" }), route("bad-date", 20, { quote_expires_at: "invalid" }),
    route("pair", 20, { source_currency: "ETH" }), route("missing", 20, { source_amount: undefined, source_amount_minor: 100 }),
    route("zero", 20, { source_amount: "0" }), route("negative", -1), route("nan", 20, { target_amount: "NaN" }),
    route("infinite", 20, { target_amount: "Infinity" }),
  ];
  assert.equal(routeReferenceRate(invalid, "BTC", "USD", now), null);
  assert.deepEqual(routeReferenceRate([...invalid, route("valid", 25, { quote_expires_at: "2026-10-30T12:00:01Z" })], "BTC", "USD", now), { rate: 25, sample_count: 1 });
});

test("updates replace duplicate route ids and the whole result set contributes", () => {
  const routes = [...Array.from({ length: 101 }, (_, i) => route(String(i), i + 1)), route("0", 1000)];
  assert.deepEqual(routeReferenceRate(routes, "BTC", "USD", now), { rate: 52, sample_count: 101 });
  assert.equal(routeReferenceRate([], "BTC", "USD", now), null);
});

test("format preserves tiny crypto rates and localizes numbers", () => {
  assert.equal(formatReferenceRate(0.00000000123456789, "en"), "0.0000000012345679");
  assert.equal(formatReferenceRate(370.25, "ru"), "370,25");
});
