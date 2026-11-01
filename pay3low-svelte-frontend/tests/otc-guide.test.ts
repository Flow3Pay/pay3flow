import assert from "node:assert/strict";
import test from "node:test";
import { buildOtcFrames, otcOfferCovers, otcSide, otcTerminalHash } from "../src/lib/guides/otc/frames.ts";
import { otcCorridor, routeUsesOtc } from "../src/lib/otc/route.ts";

const copy = (key: string, params: Record<string, string | number> = {}) => key.replace(/\{(\w+)\}/g, (_, name: string) => String(params[name] ?? `{${name}}`));
const offer = (ever: string, usdt: string) => ({ terms: { ever_amount: ever, usdt_amount: usdt } });

test("the OTC side follows the asset the traveller sends", () => {
  assert.equal(otcSide({ from: "USDT" }), "sell");
  assert.equal(otcSide({ from: "EVER" }), "buy");
  assert.equal(otcTerminalHash({ from: "USDT" }), "#/otc?market=EVER-USDT&side=sell");
  assert.equal(otcTerminalHash({ from: "usdt" }), "#/otc?market=EVER-USDT&side=sell");
});

test("an offer only covers the leg when its own size pays for the whole amount", () => {
  const buying = { from: "USDT", to: "EVER", inputAmount: "100" };
  const selling = { from: "EVER", to: "USDT", inputAmount: "250" };
  assert.equal(otcOfferCovers(offer("10", "9.7"), buying), false, "a $10 offer is not a $100 fill");
  assert.equal(otcOfferCovers(offer("10", "100"), buying), true);
  assert.equal(otcOfferCovers(offer("300", "290"), selling), true);
  assert.equal(otcOfferCovers(offer("249.9", "240"), selling), false);
  assert.equal(otcOfferCovers(offer("1", "1"), { from: "USDT", to: "EVER" }), true, "an unknown amount asks nothing of the offer");
});

test("the OTC walkthrough walks the live book, the two wallets and the settlement", () => {
  const buy = buildOtcFrames({ from: "USDT", to: "EVER", amount: "100 USDT", output: "1030 EVER" }, copy);
  const sell = buildOtcFrames({ from: "EVER", to: "USDT", amount: "1030 EVER", output: "100 USDT" }, copy);
  assert.deepEqual(buy.map(frame => frame.kind), ["open", "review", "verify", "act", "receive"]);
  assert.match(buy[0].text, /USDT.*EVER/);
  assert.match(buy[2].text, /seller's EVER wallet/);
  assert.match(sell[2].text, /buyer's EVER receiving wallet/);
  assert.match(buy[4].text, /EVER arrived/);
  assert.match(sell[4].text, /USDT arrived/);
  assert.ok([...buy, ...sell].every(frame => !frame.title.includes("{") && !frame.text.includes("{")));
});

test("routes are marked as OTC only when our own desk is one of their venues", () => {
  assert.equal(routeUsesOtc({ route_provider: "pay3flow-otc", legs: [], cycle_legs: undefined }), true);
  assert.equal(routeUsesOtc({ route_provider: "binance", legs: [{ provider: "Pay3Flow-OTC" } as never], cycle_legs: undefined }), true);
  assert.equal(routeUsesOtc({ route_provider: "binance", legs: [{ provider: "okx" } as never], cycle_legs: [{ provider: "symbiosis" }] }), false);
  assert.equal(otcCorridor("byn", "ever"), false);
  assert.equal(otcCorridor("EVER", "usdt"), true);
  assert.equal(otcCorridor("USDT", "USDT"), false);
});
