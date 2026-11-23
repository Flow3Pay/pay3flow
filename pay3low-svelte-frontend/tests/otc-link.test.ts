import assert from "node:assert/strict";
import test from "node:test";
import { otcHash, parseOtcHash } from "../src/lib/otc/link.ts";

test("OTC links round-trip direction, type, exact decimals and supported settlement networks", () => {
  const state = { marketId: "EVER-USDT", side: "sell", type: "limit", amount: "100.250000", price: "0.012340", sendNetworkId: "everscale", receiveNetworkId: "polygon-pos" } as const;
  assert.deepEqual(parseOtcHash(otcHash(state)), state);
  assert.deepEqual(parseOtcHash(otcHash({ ...state, type: "market", amount: "", price: "" })), { ...state, type: "market", amount: "", price: "" });
  assert.equal(parseOtcHash(otcHash({ ...state, amount: "1,25" })).amount, "1.25");
});

test("legacy OTC links cannot restore TRON selections", () => {
  assert.deepEqual(parseOtcHash("#/otc?market=EVER-USDT&sendNetwork=tron&receiveNetwork=tron"), {
    marketId: "EVER-USDT",
  });
});

test("old market-only links remain usable and malformed parameters are ignored", () => {
  assert.deepEqual(parseOtcHash("#/otc?market=ETH-USDT"), { marketId: "ETH-USDT" });
  assert.deepEqual(parseOtcHash("#/otc"), {});
  assert.deepEqual(parseOtcHash("#/swap?market=BTC-USDT&amount=100"), {});
  assert.deepEqual(parseOtcHash("#/otc?market=%3Cscript%3E&side=short&type=instant&amount=1e10&price=Infinity&sendNetwork=../../x&receiveNetwork=%3Csvg%3E"), {});
  for (const value of ["-1", "NaN", "1.2.3", "1000000000001", "1".repeat(100)]) {
    assert.equal(parseOtcHash(`#/otc?amount=${value}&price=${value}`).amount, undefined);
    assert.equal(parseOtcHash(`#/otc?amount=${value}&price=${value}`).price, undefined);
  }
});
