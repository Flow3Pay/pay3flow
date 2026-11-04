import { test } from "node:test";
import assert from "node:assert/strict";
import { candleIntervalLabel, chartRangeLabel, chartRanges, chartWindow, chartWindows, markCandles, percentChange, quoteObservations, tradeRow, trimDecimal } from "../src/lib/otc/market.ts";
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

test("fallback candles keep the selected range window and grid", () => {
  const now = Date.parse("2026-11-03T12:00:00Z");
  const marks = [
    { time: now - 2 * 3600_000, value: 0.1 },
    { time: now - 10 * 60_000 - 40_000, value: 0.2 },
    { time: now - 10 * 60_000 - 20_000, value: 0.3 },
  ];
  const hour = markCandles(marks, chartWindow("1h", now));
  const day = markCandles(marks, chartWindow("24h", now));
  // 1h drops the two-hour-old print and both recent prints share a one-minute bucket.
  assert.equal(hour.length, 1);
  assert.deepEqual({ open: hour[0].open, high: hour[0].high, low: hour[0].low, close: hour[0].close }, { open: 0.2, high: 0.3, low: 0.2, close: 0.3 });
  // 24h keeps the older print and keeps the two recent prints in one 15-minute bucket.
  assert.equal(day.length, 2);
  assert.equal(day[0].open, 0.1);
  assert.deepEqual({ open: day[1].open, close: day[1].close }, { open: 0.2, close: 0.3 });
});

test("each range uses its own server bucket layout", () => {
  const now = Date.parse("2026-11-03T12:00:00Z");
  assert.deepEqual(chartWindows["1h"], { bucketSeconds: 60, buckets: 60 });
  assert.deepEqual(chartWindows["1y"], { bucketSeconds: 604_800, buckets: 52 });
  const year = chartWindow("1y", now);
  const bucketMs = year.bucketSeconds * 1000;
  // The window ends on its bucket boundary, not at the raw wall-clock instant.
  assert.equal(year.from + year.bucketSeconds * year.buckets * 1000, Math.floor(now / bucketMs) * bucketMs);
});

test("every range keeps its own grid, interval and caption", () => {
  const now = Date.parse("2026-11-03T12:00:07.500Z");
  const expected = {
    "1h": ["1 hour", "1m", 60, 60],
    "24h": ["24 hours", "15m", 900, 96],
    "1w": ["week", "2h", 7_200, 84],
    "1m": ["month", "6h", 21_600, 120],
    "6m": ["6 months", "1D", 86_400, 183],
    "1y": ["year", "1W", 604_800, 52],
  } as const;
  for (const range of chartRanges) {
    const [label, interval, bucketSeconds, buckets] = expected[range];
    const window = chartWindow(range, now);
    assert.equal(chartRangeLabel(range), label);
    assert.equal(candleIntervalLabel(window.bucketSeconds), interval);
    assert.equal(window.bucketSeconds, bucketSeconds);
    assert.equal(window.buckets, buckets);
    const bucketMs = bucketSeconds * 1000;
    // Both edges sit on the bucket grid, so the newest candle is never partial.
    assert.equal(window.from % bucketMs, 0);
    assert.equal(window.from + window.bucketSeconds * window.buckets * 1000, Math.floor(now / bucketMs) * bucketMs);
  }
  // 24h must read as minutes, never as the bare span klinecharts defaults to.
  assert.equal(candleIntervalLabel(900), "15m");
  assert.equal(chartRangeLabel("6m", "ru"), "6 месяцев");
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
