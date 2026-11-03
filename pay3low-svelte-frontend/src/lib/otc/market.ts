import type { Direction, Terms, Trade } from "./service";

/** Exact decimal text, matching the booking service without runtime coupling. */
function decimal(units: string, precision: number): string {
  const amount = BigInt(units), scale = 10n ** BigInt(precision);
  const fraction = (amount % scale).toString().padStart(precision, "0").replace(/0+$/, "");
  return `${amount / scale}${fraction ? `.${fraction}` : ""}`;
}

/** Exact USDT-per-EVER rate from the immutable booked terms. */
function rate(terms: Terms): string {
  const ever = BigInt(terms.direction === "buy" ? terms.output_units : terms.input_units);
  const usdt = BigInt(terms.fee_policy.basis_units);
  return decimal((usdt * 1_000_000_000n / ever).toString(), 6);
}

/** A single server-recorded desk quote. Amounts stay exact until drawing. */
export interface QuoteObservation {
  id: string;
  time: number;
  direction: Direction;
  price: string;
  priceValue: number;
  amount: string;
  amountValue: number;
  amountUnits: bigint;
  total: string;
  totalValue: number;
  totalUnits: bigint;
}

export const chartRanges = ["1h", "24h", "1w", "1m", "6m", "1y"] as const;
export type ChartRange = (typeof chartRanges)[number];

/** Bucket layout used by the server for each range; the window is `buckets * bucketSeconds`. */
export const chartWindows: Record<ChartRange, { bucketSeconds: number; buckets: number }> = {
  "1h": { bucketSeconds: 60, buckets: 60 },
  "24h": { bucketSeconds: 900, buckets: 96 },
  "1w": { bucketSeconds: 7_200, buckets: 84 },
  "1m": { bucketSeconds: 21_600, buckets: 120 },
  "6m": { bucketSeconds: 86_400, buckets: 183 },
  "1y": { bucketSeconds: 604_800, buckets: 52 },
};

export interface ChartWindow {
  /** Window start in milliseconds. */
  from: number;
  bucketSeconds: number;
  buckets: number;
}

export interface MarketMark {
  time: number;
  value: number;
}

export interface MarketCandle {
  timestamp: number;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
  [key: string]: unknown;
}

/** The exact window the chart should cover, ending at `now` (milliseconds). */
export function chartWindow(range: ChartRange, now = Date.now()): ChartWindow {
  const { bucketSeconds, buckets } = chartWindows[range];
  return { from: now - bucketSeconds * buckets * 1000, bucketSeconds, buckets };
}

/**
 * Re-bucket a window so a coarser source (for example daily spot points) still
 * covers the whole selected range instead of leaving empty space between bars.
 */
export function fitWindow(window: ChartWindow, minimumBucketSeconds: number): ChartWindow {
  const bucketSeconds = Math.max(window.bucketSeconds, Math.max(1, minimumBucketSeconds));
  const span = window.bucketSeconds * window.buckets;
  return { from: window.from, bucketSeconds, buckets: Math.max(1, Math.ceil(span / bucketSeconds)) };
}

/**
 * Turns prints into range-scoped OHLC candles. Prints outside the window are
 * dropped and the rest are grouped on the same grid the server uses, so every
 * range keeps its own granularity instead of replaying the whole history.
 */
export function markCandles(marks: MarketMark[], window: ChartWindow): MarketCandle[] {
  const bucketMs = window.bucketSeconds * 1000;
  const to = window.from + bucketMs * window.buckets;
  const candles: MarketCandle[] = [];
  for (const mark of marks) {
    if (!(mark.time >= window.from && mark.time < to)) continue;
    const timestamp = window.from + Math.floor((mark.time - window.from) / bucketMs) * bucketMs;
    const candle = candles.at(-1);
    if (candle?.timestamp === timestamp) {
      candle.high = Math.max(candle.high, mark.value);
      candle.low = Math.min(candle.low, mark.value);
      candle.close = mark.value;
    } else {
      candles.push({ timestamp, open: mark.value, high: mark.value, low: mark.value, close: mark.value, volume: 0 });
    }
  }
  return candles;
}

/** Price observations are the participant's own server-confirmed quotes. */
export function quoteObservations(trades: Trade[]): QuoteObservation[] {
  return trades
    .flatMap((trade): QuoteObservation[] => {
      const terms = trade.terms;
      const time = Date.parse(trade.created_at ?? "");
      if (!terms || !Number.isFinite(time)) return [];
      const price = rate(terms);
      const priceValue = Number(price);
      const usdtUnits = BigInt(terms.direction === "buy" ? terms.input_units : terms.output_units);
      const everUnits = BigInt(terms.direction === "buy" ? terms.output_units : terms.input_units);
      const amountValue = Number(decimal(everUnits.toString(), 9));
      const totalValue = Number(decimal(usdtUnits.toString(), 6));
      if (!(priceValue > 0) || !Number.isFinite(amountValue) || !Number.isFinite(totalValue)) return [];
      return [{
        id: trade.id,
        time,
        direction: terms.direction,
        price,
        priceValue,
        amount: decimal(everUnits.toString(), 9),
        amountValue,
        amountUnits: everUnits,
        total: decimal(usdtUnits.toString(), 6),
        totalValue,
        totalUnits: usdtUnits,
      }];
    })
    .sort((a, b) => a.time - b.time);
}

export function toNumber(value: bigint): number {
  return Number(value);
}

export function percentChange(observations: QuoteObservation[]): number | null {
  if (observations.length < 2) return null;
  const first = observations[0].priceValue;
  const last = observations.at(-1)!.priceValue;
  return first > 0 ? (last - first) / first * 100 : null;
}

export function trimDecimal(value: string, maximumFractionDigits = 4): string {
  if (!value.includes(".")) return value;
  const [whole, fraction] = value.split(".");
  const trimmed = fraction.slice(0, maximumFractionDigits).replace(/0+$/, "");
  return `${whole}${trimmed ? `.${trimmed}` : ""}`;
}

export interface TradeRow {
  price: string;
  amount: string;
  total: string;
  direction: Direction;
  time: number | null;
}

export function tradeRow(trade: Trade): TradeRow {
  const terms = trade.terms;
  const usdtUnits = BigInt(terms.direction === "buy" ? terms.input_units : terms.output_units);
  const everUnits = BigInt(terms.direction === "buy" ? terms.output_units : terms.input_units);
  return {
    price: rate(terms),
    amount: decimal(everUnits.toString(), 9),
    total: decimal(usdtUnits.toString(), 6),
    direction: terms.direction,
    time: trade.created_at ? Date.parse(trade.created_at) : null,
  };
}
