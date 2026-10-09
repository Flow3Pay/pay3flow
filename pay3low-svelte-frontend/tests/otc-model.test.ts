import assert from "node:assert/strict";
import test from "node:test";
import { demoSnapshot, formatPrice, marketFromHash, markets, prepareDemoOrder } from "../src/lib/otc/model.ts";
const market = markets.find((item) => item.id === "BTC-USDT")!;
const snapshot = demoSnapshot(market);

test("limit orders respect source amounts and base precision in both directions", () => {
  const buy = prepareDemoOrder(market, snapshot, "buy", "limit", "1,000", "60000");
  assert.ok(buy.draft!.total <= 1); // A comma is a decimal separator, never a thousands separator.
  const draft = prepareDemoOrder(market, snapshot, "buy", "limit", "1000", "60000").draft!;
  assert.equal(draft.amount, .016666);
  assert.ok(draft.total <= 1000);
  const sell = prepareDemoOrder(market, snapshot, "sell", "limit", "0.25", "60000").draft!;
  assert.equal(sell.total, 15000);
});

test("market orders consume opposite levels and calculate a weighted price", () => {
  const book = { ...snapshot, asks: [{ price: 100, amount: 2, total: 200, depth: 200 }, { price: 200, amount: 3, total: 600, depth: 800 }], bids: [{ price: 90, amount: 1, total: 90, depth: 90 }, { price: 80, amount: 3, total: 240, depth: 330 }] };
  const buy = prepareDemoOrder(market, book, "buy", "market", "400", "ignored").draft!;
  assert.equal(buy.amount, 3);
  assert.equal(buy.total, 400);
  assert.equal(buy.price, 400 / 3);
  const sell = prepareDemoOrder(market, book, "sell", "market", "2", "ignored").draft!;
  assert.equal(sell.amount, 2);
  assert.equal(sell.total, 170);
  assert.equal(sell.price, 85);
  assert.equal(prepareDemoOrder(market, book, "buy", "market", "801", "").error, "liquidity");
  assert.equal(prepareDemoOrder(market, book, "sell", "market", "4.1", "").error, "liquidity");
});

test("malformed, tiny and unbounded values never produce an order", () => {
  for (const value of ["", "0", "-5", "NaN", "Infinity", "1e8", "1.2.3"]) assert.equal(prepareDemoOrder(market, snapshot, "buy", "limit", value, "60000").draft, null);
  assert.equal(prepareDemoOrder(market, snapshot, "buy", "limit", "0.000001", "60000").error, "precision");
  assert.equal(prepareDemoOrder(market, snapshot, "buy", "limit", "1000", "0").error, "price");
  assert.equal(prepareDemoOrder(market, snapshot, "buy", "limit", "1000000000001", "60000").error, "range");
});

test("shared market selection validates IDs and falls back safely", () => {
  assert.equal(marketFromHash("#/otc").id, "EVER-USDT");
  assert.equal(marketFromHash("#/otc?market=EVER-USDT").base, "EVER");
  assert.equal(marketFromHash("#/otc?market=ETH-USDT").base, "ETH");
  assert.equal(marketFromHash("#/otc?market=BTC-USDT").base, "BTC");
  assert.equal(marketFromHash("#/otc?market=missing").base, "EVER");
});

test("EVER orders retain small quote prices and base amounts in both directions", () => {
  const ever = marketFromHash("#/otc");
  const book = demoSnapshot(ever);
  assert.equal(formatPrice(book.asks[0].price, ever), "0.01001");
  assert.equal(formatPrice(book.bids[0].price, ever), "0.00999");
  const buy = prepareDemoOrder(ever, book, "buy", "limit", "1", "0.01234").draft!;
  assert.equal(buy.marketId, "EVER-USDT");
  assert.equal(buy.amount, 81.037277);
  assert.ok(buy.total <= 1);
  const sell = prepareDemoOrder(ever, book, "sell", "limit", "100.25", "0.01234").draft!;
  assert.equal(sell.amount, 100.25);
  assert.ok(Math.abs(sell.total - 1.237085) < 1e-10);
});
