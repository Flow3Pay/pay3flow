import { test } from "node:test";
import assert from "node:assert/strict";
import { candleIntervalLabel, chartRanges, chartWindow, chartWindows, markCandles, percentChange, quoteObservations, tradeRow, trimDecimal } from "../src/lib/otc/market.ts";
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

test("fallback candles keep the selected timeframe window and grid", () => {
  const now = Date.parse("2026-11-03T12:00:00Z");
  const marks = [
    { time: now - 4 * 3600_000, value: 0.1 },
    { time: now - 20_000, value: 0.2 },
    { time: now - 10_000, value: 0.3 },
  ];
  const fine = markCandles(marks, chartWindow("30s", now));
  const hour = markCandles(marks, chartWindow("1h", now));
  // 30s drops the four-hour-old print and both recent prints share one 30s bucket.
  assert.equal(fine.length, 1);
  assert.deepEqual({ open: fine[0].open, high: fine[0].high, low: fine[0].low, close: fine[0].close }, { open: 0.2, high: 0.3, low: 0.2, close: 0.3 });
  // 1h keeps the older print and keeps the two recent prints in one hour bucket.
  assert.equal(hour.length, 2);
  assert.equal(hour[0].open, 0.1);
  assert.deepEqual({ open: hour[1].open, close: hour[1].close }, { open: 0.2, close: 0.3 });
});

test("each timeframe uses its own server bucket layout", () => {
  const now = Date.parse("2026-11-03T12:00:00Z");
  assert.deepEqual(chartWindows["30s"], { bucketSeconds: 30, buckets: 300 });
  assert.deepEqual(chartWindows["1d"], { bucketSeconds: 86_400, buckets: 300 });
  const day = chartWindow("1d", now);
  const bucketMs = day.bucketSeconds * 1000;
  // The window ends on its bucket boundary, not at the raw wall-clock instant.
  assert.equal(day.from + day.bucketSeconds * day.buckets * 1000, Math.floor(now / bucketMs) * bucketMs);
});

test("every timeframe draws candles of its own size", () => {
  const now = Date.parse("2026-11-03T12:00:07.500Z");
  const expected = {
    "30s": ["30s", 30, 300],
    "1m": ["1m", 60, 300],
    "3m": ["3m", 180, 300],
    "5m": ["5m", 300, 300],
    "30m": ["30m", 1_800, 300],
    "1h": ["1h", 3_600, 300],
    "4h": ["4h", 14_400, 300],
    "8h": ["8h", 28_800, 300],
    "16h": ["16h", 57_600, 300],
    "1d": ["1d", 86_400, 300],
  } as const;
  for (const range of chartRanges) {
    const [interval, bucketSeconds, buckets] = expected[range];
    const window = chartWindow(range, now);
    assert.equal(candleIntervalLabel(window.bucketSeconds), interval);
    assert.equal(window.bucketSeconds, bucketSeconds);
    assert.equal(window.buckets, buckets);
    const bucketMs = bucketSeconds * 1000;
    // Both edges sit on the bucket grid, so the newest candle is never partial.
    assert.equal(window.from % bucketMs, 0);
    assert.equal(window.from + window.bucketSeconds * window.buckets * 1000, Math.floor(now / bucketMs) * bucketMs);
  }
  // Minutes must read as minutes, never as the bare span klinecharts defaults to.
  assert.equal(candleIntervalLabel(900), "15m");
  assert.equal(candleIntervalLabel(900, "ru"), "15 мин");
});

test("changes and order rows stay deterministic", () => {
  const observations = quoteObservations([
    trade("a", "buy", "2026-09-01T10:00:00Z", "100", "500"),
    trade("b", "buy", "2026-10-24T10:00:00Z", "100", "400"),
  ]);
  assert.ok((percentChange(observations) ?? 0) > 0);
  const row = tradeRow(trade("b", "buy", "2026-10-24T10:00:00Z", "100", "400"));
  assert.deepEqual({ price: row.price, amount: row.amount, total: row.total }, { price: "0.25", amount: "400", total: "100" });
  assert.equal(trimDecimal("0.123456789", 5), "0.12345");
});
