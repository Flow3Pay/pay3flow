import assert from "node:assert/strict";
import test from "node:test";
import { buildCifraFrames, cifraTrade } from "../src/lib/guides/cifra/frames.ts";
const copy = (key: string, params: Record<string, string | number> = {}) => key.replace(/\{(\w+)\}/g, (_, key) => String(params[key] ?? `{${key}}`));

test("Cifra order side follows the IMEX base and quote and normalizes the ruble code", () => {
  assert.deepEqual(cifraTrade({ kind: "buy", from: "RUB", to: "USDT" }), { base: "USDT", quote: "RUB", buying: true, ticker: "USDT-RUB.IMEX", field: "Sum" });
  assert.deepEqual(cifraTrade({ kind: "swap", pair: "USDT-RUR.IMEX", from: "USDT", to: "RUB" }), { base: "USDT", quote: "RUB", buying: false, ticker: "USDT-RUB.IMEX", field: "Quantity" });
  assert.equal(cifraTrade({ kind: "swap", pair: "ETHUSDT", from: "USDT", to: "ETH" })?.buying, true);
  assert.equal(cifraTrade({ kind: "swap", pair: "ETHUSDT", from: "ETH", to: "USDT" })?.buying, false);
  assert.equal(cifraTrade({ kind: "swap", pair: "SOLUSDT", from: "ETH", to: "USDT" }), null);
  assert.equal(cifraTrade({ kind: "swap", from: "ETH", to: "USDT" }), null);
  assert.equal(cifraTrade({ kind: "transfer", from: "USDT", to: "USDT" }), null);
});

test("Cifra walkthrough changes funding, order entry and the receiving balance with the route", () => {
  const buy = buildCifraFrames({ kind: "buy", from: "RUB", to: "USDT", amount: "10000 RUB" }, copy);
  const sell = buildCifraFrames({ kind: "sell", from: "USDT", to: "RUB", network: "TRON (TRC-20)" }, copy);
  assert.equal(buy.length, 6);
  assert.match(buy[0].text, /advanced verification/);
  assert.match(buy[1].text, /bank payment method/);
  assert.match(buy[2].text, /USDT-RUB.IMEX/);
  assert.match(buy[3].text, /Sum tab.*RUB/);
  assert.match(buy[4].text, /Place a buy order/);
  assert.match(buy[5].text, /actual fills.*USDT balance/);
  assert.match(sell[1].text, /match the network/);
  assert.match(sell[3].text, /Quantity tab.*USDT/);
  assert.match(sell[4].text, /Place a sell order/);
  assert.match(sell[5].text, /RUB balance.*Withdraw money/);
  assert.ok([...buy, ...sell].every(frame => !frame.text.includes("{")));
});
