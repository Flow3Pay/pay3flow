import { test } from "node:test";
import assert from "node:assert/strict";
import { guideRoutePath, parseExchangeHash } from "../src/lib/guide-link.ts";
import type { RouteCandidate } from "../src/lib/exchange";

test("legacy TON guide signatures match GRAM assets without renaming providers or networks", () => {
  const legacy = {
    source_currency: "TON", target_currency: "USDC", entry_asset: "TON", entry_network: "ton",
    route_kind: "crypto_to_crypto", route_provider: "TON", legs: [{ kind: "entry", provider: "TON" }],
    entry_offer_snapshot: { source: "TON", advertiser: { id: "TON" }, fiat: "USD", asset: "TON", network: "ton" },
    route_path: ["TON@ton", "USDT@ethereum", "USDC@ethereum"],
    market_path: { venue: "TON", source_pair: "TONUSDT", target_pair: "USDTUSDC" },
    cycle_legs: [{ provider: "TON", from_asset: "TON@ton", to_asset: "USDT@ethereum", market_pair: "TONUSDT" }],
  } as unknown as RouteCandidate;
  const current = {
    ...legacy, source_currency: "GRAM", entry_asset: "GRAM",
    entry_offer_snapshot: { ...legacy.entry_offer_snapshot, asset: "GRAM" },
    route_path: ["GRAM@ton", "USDT@ethereum", "USDC@ethereum"],
    market_path: { ...legacy.market_path, source_pair: "GRAMUSDT" },
    cycle_legs: [{ ...legacy.cycle_legs![0], from_asset: "GRAM@ton", market_pair: "GRAMUSDT" }],
  } as RouteCandidate;
  const params = new URLSearchParams({ path: guideRoutePath(legacy), view: "guide" });
  const parsed = parseExchangeHash(`#/guide/TON/USDC?${params}`)!;
  assert.equal(parsed.sourceCurrency, "GRAM");
  assert.equal(parsed.params.get("path"), guideRoutePath(current));
  assert.equal(parsed.params.get("view"), "guide");
});

test("direct TON destination guide signatures restore and malformed signatures remain inert", () => {
  const legacy = { source_currency: "ETH", target_currency: "TON", entry_asset: "TON", entry_network: "ethereum", route_kind: "crypto_to_crypto", route_provider: "near-intents", legs: [], route_path: ["ETH@ethereum", "TON@ton"] } as unknown as RouteCandidate;
  const current = { ...legacy, target_currency: "GRAM", entry_asset: "GRAM", route_path: ["ETH@ethereum", "GRAM@ton"] };
  const params = new URLSearchParams({ path: guideRoutePath(legacy) });
  assert.equal(parseExchangeHash(`#/guide/ETH/TON?${params}`)!.params.get("path"), guideRoutePath(current));
  assert.equal(parseExchangeHash("#/guide/ETH/TON?path=%5Bbad")!.params.get("path"), "[bad");
});
