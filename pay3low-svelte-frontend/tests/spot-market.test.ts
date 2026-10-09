import assert from "node:assert/strict";
import test from "node:test";
import { buildSpotFrames, isSpotStep, spotMarket } from "../src/lib/guides/spot/frames.ts";

const copy = (key: string, params: Record<string, string | number> = {}) => key.replace(/\{(\w+)\}/g, (_, name: string) => String(params[name] ?? `{${name}}`));

test("spot order side follows the base asset across venue symbol formats", () => {
  for (const pair of ["BTCUSDT", "BTC/USDT", "BTC_USDT", "btc-usdt"]) {
    assert.deepEqual(spotMarket({ pair, from: "USDT", to: "BTC" }), { base: "BTC", quote: "USDT", buying: true });
    assert.deepEqual(spotMarket({ pair, from: "BTC", to: "USDT" }), { base: "BTC", quote: "USDT", buying: false });
  }
  assert.deepEqual(spotMarket({ pair: "0G_USDT", from: "USDT", to: "0G" }), { base: "0G", quote: "USDT", buying: true });
  assert.equal(spotMarket({ pair: "BTCUSDT", from: "ETH", to: "USDT" }), undefined);
});

test("spot illustration requires a supported venue and a matching market", () => {
  const step = { kind: "swap" as const, pair: "AIUSDT", from: "USDT", to: "AI", provider: "bitget" };
  assert.equal(isSpotStep(step), true);
  assert.equal(isSpotStep({ ...step, provider: "whitebird" }), true);
  assert.equal(isSpotStep({ ...step, pair: undefined }), false);
  assert.equal(isSpotStep({ ...step, pair: "BTCUSDT" }), false);
  assert.equal(isSpotStep({ ...step, kind: "transfer" }), false);
  assert.equal(isSpotStep({ ...step, provider: "cow-swap" }), false);
});

