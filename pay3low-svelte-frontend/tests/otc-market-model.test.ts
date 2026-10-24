import { test } from "node:test";
import assert from "node:assert/strict";
import { bookLevels, bookSummary, candleSeries, filterByRange, percentChange, quoteObservations, rangeBucketMs, tradeRow, trimDecimal } from "../src/lib/otc/market.ts";
import type { Trade } from "../src/lib/otc/service.ts";

function units(value: string, precision: number) {
  const [whole, fraction = ""] = value.split(".");
  return (BigInt(whole) * 10n ** BigInt(precision) + BigInt(fraction.padEnd(precision, "0") || "0")).toString();
}
function trade(id: string, direction: "buy" | "sell", createdAt: string, input: string, output: string) {
  const usdt = direction === "buy" ? input : output;
  const ever = direction === "buy" ? output : input;
  return {
    id,
    created_at: createdAt,
    terms: {
      direction,
      input,
      output,
      input_units: units(input, direction === "buy" ? 6 : 9),
      output_units: units(output, direction === "buy" ? 9 : 6),
      fee_policy: { basis_units: units(usdt, 6), fee_units: "5000000" },
    },
  } as unknown as Trade;
}

test("quote observations use exact USDT and EVER legs", () => {
  const observations = quoteObservations([
    trade("b", "buy", "2026-10-24T10:00:00Z", "100", "500"),
    trade("s", "sell", "2026-10-24T09:00:00Z", "500", "50"),
  ]);
  assert.deepEqual(observations.map(item => item.id), ["s", "b"]);
  assert.equal(observations[0].price, "0.1");
  assert.equal(observations[0].amount, "500");
  assert.equal(observations[0].total, "50");
  assert.equal(observations[1].amount, "500");
  assert.equal(observations[1].total, "100");
});

test("book groups equal prices, accumulates depth and orders best first", () => {
  const observations = quoteObservations([
    trade("b1", "buy", "2026-10-24T10:00:00Z", "100", "500"),
    trade("b2", "buy", "2026-10-24T10:01:00Z", "50", "250"),
    trade("s1", "sell", "2026-10-24T10:02:00Z", "500", "45"),
  ]);
  const asks = bookLevels(observations, "buy");
  const bids = bookLevels(observations, "sell");
  assert.equal(asks.length, 1);
  assert.equal(asks[0].amountValue, 750);
  assert.equal(asks[0].totalValue, 150);
  assert.equal(asks[0].count, 2);
  assert.equal(bids[0].price, "0.09");
  const summary = bookSummary(observations);
  assert.equal(summary.bestAsk, 0.2);
  assert.equal(summary.bestBid, 0.09);
  assert.ok(summary.spread && Math.abs(summary.spread - 0.11) < 1e-12);
  assert.equal(summary.mid, "0.09");
});

test("candles aggregate server observations into time buckets", () => {
  const observations = quoteObservations([
    trade("a", "buy", "2026-10-24T10:00:00Z", "100", "500"),
    trade("b", "buy", "2026-10-24T10:02:00Z", "100", "400"),
    trade("c", "sell", "2026-10-24T10:04:00Z", "500", "45"),
  ]);
  const candles = candleSeries(observations, rangeBucketMs("1D"));
  assert.equal(candles.length, 1);
  assert.equal(candles[0].open, 0.2);
  assert.equal(candles[0].close, 0.09);
  assert.equal(candles[0].high, 0.25);
  assert.equal(candles[0].low, 0.09);
  assert.equal(candles[0].count, 3);
  assert.equal(candles[0].volume, 1400);
});

test("range filtering, changes and order rows stay deterministic", () => {
  const observations = quoteObservations([
    trade("a", "buy", "2026-09-01T10:00:00Z", "100", "500"),
    trade("b", "buy", "2026-10-24T10:00:00Z", "100", "400"),
  ]);
  assert.equal(filterByRange(observations, "1D", Date.parse("2026-10-24T12:00:00Z")).length, 1);
  assert.equal(filterByRange(observations, "ALL").length, 2);
  assert.ok((percentChange(observations) ?? 0) > 0);
  const row = tradeRow(trade("b", "buy", "2026-10-24T10:00:00Z", "100", "400"));
  assert.deepEqual({ price: row.price, amount: row.amount, total: row.total }, { price: "0.25", amount: "400", total: "100" });
  assert.equal(trimDecimal("0.123456789", 5), "0.12345");
});
