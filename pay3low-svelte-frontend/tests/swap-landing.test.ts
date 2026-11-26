import { test } from "node:test";
import assert from "node:assert/strict";
import { assetDisplayName, isPopularSwapPair, relatedSwapPairs, popularSwapPairs } from "../src/lib/swap-landing.ts";

test("asset names resolve known tickers and fall back to the ticker", () => {
  assert.equal(assetDisplayName("btc"), "Bitcoin");
  assert.equal(assetDisplayName("USDT"), "Tether USD");
  assert.equal(assetDisplayName("XYZ"), "XYZ");
});

test("related pairs share a side and never repeat the current direction", () => {
  const related = relatedSwapPairs("USDT", "BTC");
  assert.equal(related.length, 6);
  assert.ok(!related.some(([source, target]) => source === "USDT" && target === "BTC"));
  assert.ok(related.some(([source, target]) => source === "USDT" || target === "BTC"));
  assert.equal(new Set(related.map(([source, target]) => `${source}/${target}`)).size, related.length);
});

test("every curated pair has both directions available", () => {
  assert.ok(popularSwapPairs.length >= 20);
  for (const [source, target] of popularSwapPairs) assert.notEqual(source, target);
});

test("only curated directions are marked indexable", () => {
  assert.equal(isPopularSwapPair("USDT", "BTC"), true);
  assert.equal(isPopularSwapPair("FOO", "BAR"), false);
});
