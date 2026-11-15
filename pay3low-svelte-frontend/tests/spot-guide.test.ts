import { test } from "node:test";
import assert from "node:assert/strict";
import { spotTrade, buildSpotFrames, spotProfiles, spotTutorialUrl } from "../src/lib/guides/spot/frames.ts";
const copy = (key: string, params: Record<string, string | number> = {}) => key.replace(/\{(\w+)\}/g, (match, name) => String(params[name] ?? match));

test("spot direction follows the market base and quote for arbitrary crypto pairs", () => {
  assert.deepEqual(spotTrade("BTCUSDT", "USDT", "BTC"), { base: "BTC", quote: "USDT", side: "buy", label: "BTC / USDT" });
  assert.deepEqual(spotTrade("eth_btc", "eth", "btc"), { base: "ETH", quote: "BTC", side: "sell", label: "ETH / BTC" });
  assert.deepEqual(spotTrade("BTC/ETH", "ETH", "BTC"), { base: "BTC", quote: "ETH", side: "buy", label: "BTC / ETH" });
  assert.equal(spotTrade("SOLUSDC", "USDT", "BTC"), null);
  assert.equal(spotTrade(undefined, "USDT", "BTC"), null);
  assert.equal(spotTrade("USDTUSDT", "USDT", "USDT"), null);
});

test("each spot venue explains the correct spending field and actual fills", () => {
  for (const provider of ["binance", "bybit", "mexc", "bitget", "whitebird", "gate"]) {
    const buy = buildSpotFrames({ provider, venue: provider, pair: "BTCETH", from: "ETH", to: "BTC", amount: "0.5 ETH" }, copy)!;
    assert.equal(buy.length, 5);
    assert.match(buy[1].text, /select Buy: you spend ETH to receive BTC/);
    assert.ok(buy[2].text.includes(`0.5 ETH in ${spotProfiles[provider]!.buyField}`));
    assert.equal(buy[3].title, "Review and press Buy BTC");
    assert.match(buy[4].text, /actual BTC balance/);
    assert.ok(buy.every(frame => !frame.text.includes("{")), "all parameters are substituted");
    const sell = buildSpotFrames({ provider, venue: provider, pair: "ETHBTC", from: "ETH", to: "BTC", amount: "0.5 ETH" }, copy)!;
    assert.match(sell[1].text, /select Sell: you spend ETH to receive BTC/);
    assert.ok(sell[2].text.includes(`0.5 ETH in ${spotProfiles[provider]!.sellField}`));
    assert.equal(sell[3].title, "Review and press Sell ETH");
  }
});

test("Gate.io links both EVER directions to the spot pair", () => {
  const buy = buildSpotFrames({ provider: "gate", venue: "Gate.io", pair: "EVER_USDT", from: "USDT", to: "EVER", amount: "10 USDT" }, copy)!;
  const sell = buildSpotFrames({ provider: "gate", venue: "Gate.io", pair: "EVER_USDT", from: "EVER", to: "USDT", amount: "10 EVER" }, copy)!;
  assert.match(buy[1].text, /select Buy: you spend USDT to receive EVER/);
  assert.match(sell[1].text, /select Sell: you spend EVER to receive USDT/);
  assert.equal(spotTutorialUrl("gate", "EVER_USDT", "USDT", "EVER"), "https://www.gate.com/trade/EVER_USDT");
  assert.equal(spotTutorialUrl("gate", "EVER_USDT", "EVER", "USDT"), "https://www.gate.com/trade/EVER_USDT");
});

test("missing or inconsistent market metadata does not invent a trading direction", () => {
  assert.equal(buildSpotFrames({ provider: "bybit", venue: "Bybit", from: "USDT", to: "BTC" }, copy), null);
  assert.equal(buildSpotFrames({ provider: "bybit", venue: "Bybit", pair: "SOLUSDC", from: "USDT", to: "BTC" }, copy), null);
  assert.equal(buildSpotFrames({ provider: "unknown", venue: "Unknown", pair: "BTCUSDT", from: "USDT", to: "BTC" }, copy), null);
});
