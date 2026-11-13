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

/**
 * Candle timeframes the chart offers, matching the exchange convention the
 * Gate.io source uses. Each button draws candles of exactly that duration.
 */
export const chartRanges = ["30s", "1m", "3m", "5m", "30m", "1h", "4h", "8h", "16h", "1d"] as const;
export type ChartRange = (typeof chartRanges)[number];

/** Candle size on the chart grid, with the unit klinecharts exposes. */
export function candleIntervalLabel(bucketSeconds: number, locale = "en"): string {
  const ru = locale === "ru";
  if (bucketSeconds < 60) return `${Math.max(1, Math.round(bucketSeconds))}${ru ? " с" : "s"}`;
  if (bucketSeconds < 3_600) return `${Math.round(bucketSeconds / 60)}${ru ? " мин" : "m"}`;
  if (bucketSeconds < 86_400) return `${Math.round(bucketSeconds / 3_600)}${ru ? " ч" : "h"}`;
  if (bucketSeconds < 604_800) return `${Math.round(bucketSeconds / 86_400)}d`;
  return `${Math.round(bucketSeconds / 604_800)}w`;
}

/**
 * Bucket layout used by the server for each timeframe; the window is
 * `buckets * bucketSeconds`. Every timeframe keeps the provider's page of the
 * most recent candles, so a candle is always one button long.
 */
export const chartWindows: Record<ChartRange, { bucketSeconds: number; buckets: number }> = {
  "30s": { bucketSeconds: 30, buckets: 300 },
  "1m": { bucketSeconds: 60, buckets: 300 },
  "3m": { bucketSeconds: 180, buckets: 300 },
  "5m": { bucketSeconds: 300, buckets: 300 },
  "30m": { bucketSeconds: 1_800, buckets: 300 },
  "1h": { bucketSeconds: 3_600, buckets: 300 },
  "4h": { bucketSeconds: 14_400, buckets: 300 },
  "8h": { bucketSeconds: 28_800, buckets: 300 },
  "16h": { bucketSeconds: 57_600, buckets: 300 },
  "1d": { bucketSeconds: 86_400, buckets: 300 },
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

/**
 * The exact window the chart should cover, ending at `now` (milliseconds).
 * The end is aligned to a bucket boundary so it matches the server grid and
 * the newest candle is never a mixture of two periods.
 */
export function chartWindow(range: ChartRange, now = Date.now()): ChartWindow {
  const { bucketSeconds, buckets } = chartWindows[range];
  const bucketMs = bucketSeconds * 1000;
  const from = Math.floor((now - bucketMs * buckets) / bucketMs) * bucketMs;
  return { from, bucketSeconds, buckets };
}

/**
 * Turns prints into timeframe-scoped OHLC candles. Prints outside the window
 * are dropped and the rest are grouped on the same grid the server uses, so a
 * candle always spans the selected timeframe instead of replaying the whole
 * history.
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
