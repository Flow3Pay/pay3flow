import { test } from "node:test";
import assert from "node:assert/strict";
import type { P2pOffer } from "../src/lib/exchange";
import { providerRouteLegs } from "../src/lib/provider-route-legs.ts";

const offer = (source: string, asset: string) => ({ source, asset } as P2pOffer);

test("fiat entry, crypto swap and fiat exit keep all three providers", () => {
  const legs = providerRouteLegs({ source_fiat: "AMD", target_fiat: "RUB", asset: "USDT", route_provider: "symbiosis", entry_offer: offer("bncex", "USDT"), exit_offer: offer("bybit", "USDC") });
  assert.deepEqual(legs.map(({ from, to, provider }) => ({ from, to, provider })), [
    { from: "AMD", to: "USDT", provider: "bncex" },
    { from: "USDT", to: "USDC", provider: "symbiosis" },
    { from: "USDC", to: "RUB", provider: "bybit" },
  ]);
  assert.equal(legs.find(leg => leg.kind === "entry")?.provider, "bncex");
  assert.equal(legs.find(leg => leg.kind === "exit")?.provider, "bybit");
});

test("Symbiosis only labels the crypto section of fiat entry and exit routes", () => {
  for (const route of [
    { source_fiat: "RUB", target_fiat: "USDC", asset: "USDC", entry_offer: offer("bybit", "USDT") },
    { source_fiat: "USDC", target_fiat: "USD", asset: "USDT", exit_offer: offer("whitebird", "USDT") },
  ]) {
    const legs = providerRouteLegs({ ...route, route_provider: "symbiosis" });
    const swap = legs.find(leg => leg.provider === "symbiosis")!;
    assert.ok(swap);
    assert.ok(!["RUB", "USD"].includes(swap.from));
    assert.ok(!["RUB", "USD"].includes(swap.to));
    assert.equal(legs.length, 2);
  }
});

test("direct crypto swaps preserve the exact selected endpoints", () => {
  const legs = providerRouteLegs({ source_fiat: "ETH", target_fiat: "USDC", asset: "USDC", route_provider: "symbiosis" });
  assert.deepEqual(legs.map(({ from, to, provider }) => ({ from, to, provider })), [{ from: "ETH", to: "USDC", provider: "symbiosis" }]);
});
