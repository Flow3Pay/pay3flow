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

export interface BookLevel {
  price: string;
  priceValue: number;
  amount: string;
  amountValue: number;
  total: string;
  totalValue: number;
  depth: number;
  count: number;
}

export interface PriceCandle {
  time: number;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
  buyVolume: number;
  sellVolume: number;
  count: number;
}

export interface BookSummary {
  mid: string | null;
  midValue: number | null;
  bestBid: number | null;
  bestAsk: number | null;
  spread: number | null;
  spreadPercent: number | null;
  bidDepth: number;
  askDepth: number;
}

export const chartRanges = ["1D", "7D", "1M", "ALL"] as const;
export type ChartRange = (typeof chartRanges)[number];

const rangeMs: Record<Exclude<ChartRange, "ALL">, number> = {
  "1D": 86_400_000,
  "7D": 7 * 86_400_000,
  "1M": 30 * 86_400_000,
};

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

export function filterByRange(observations: QuoteObservation[], range: ChartRange, now = Date.now()): QuoteObservation[] {
  if (range === "ALL") return observations;
  return observations.filter(observation => now - observation.time <= rangeMs[range]);
}

export function toNumber(value: bigint): number {
  return Number(value);
}

/** Groups exact quote prices into one display level per side. Best price first. */
export function bookLevels(observations: QuoteObservation[], direction: Direction): BookLevel[] {
  const grouped = new Map<string, { price: string; priceValue: number; amountUnits: bigint; totalUnits: bigint; count: number }>();
  for (const observation of observations) {
    if (observation.direction !== direction) continue;
    const current = grouped.get(observation.price);
    if (current) {
      current.amountUnits += observation.amountUnits;
      current.totalUnits += observation.totalUnits;
      current.count += 1;
    } else {
      grouped.set(observation.price, {
        price: observation.price,
        priceValue: observation.priceValue,
        amountUnits: observation.amountUnits,
        totalUnits: observation.totalUnits,
        count: 1,
      });
    }
  }
  const levels = [...grouped.values()]
    .sort((a, b) => direction === "buy" ? a.priceValue - b.priceValue : b.priceValue - a.priceValue)
    .map(level => ({
      price: level.price,
      priceValue: level.priceValue,
      amount: decimal(level.amountUnits.toString(), 9),
      amountValue: Number(decimal(level.amountUnits.toString(), 9)),
      total: decimal(level.totalUnits.toString(), 6),
      totalValue: Number(decimal(level.totalUnits.toString(), 6)),
      depth: 0,
      count: level.count,
    }));
  let depth = 0;
  for (const level of levels) {
    depth += level.totalValue;
    level.depth = depth;
  }
  return levels;
}

export function bookSummary(observations: QuoteObservation[]): BookSummary {
  const asks = bookLevels(observations, "buy");
  const bids = bookLevels(observations, "sell");
  const latest = observations.at(-1) ?? null;
  const bestAsk = asks[0]?.priceValue ?? null;
  const bestBid = bids[0]?.priceValue ?? null;
  const spread = bestAsk !== null && bestBid !== null ? bestAsk - bestBid : null;
  return {
    mid: latest?.price ?? null,
    midValue: latest?.priceValue ?? null,
    bestBid,
    bestAsk,
    spread,
    spreadPercent: spread !== null && latest ? spread / latest.priceValue * 100 : null,
    bidDepth: bids.at(-1)?.depth ?? 0,
    askDepth: asks.at(-1)?.depth ?? 0,
  };
}

export function rangeBucketMs(range: ChartRange): number {
  if (range === "1D") return 5 * 60_000;
  if (range === "7D") return 60 * 60_000;
  if (range === "1M") return 6 * 60 * 60_000;
  return 24 * 60 * 60_000;
}

/** OHLC buckets are derived from quote observations, never generated locally. */
export function candleSeries(observations: QuoteObservation[], bucketMs: number): PriceCandle[] {
  const buckets = new Map<number, PriceCandle>();
  for (const observation of observations) {
    const time = Math.floor(observation.time / bucketMs) * bucketMs;
    const current = buckets.get(time);
    if (!current) {
      buckets.set(time, {
        time,
        open: observation.priceValue,
        high: observation.priceValue,
        low: observation.priceValue,
        close: observation.priceValue,
        volume: observation.amountValue,
        buyVolume: observation.direction === "buy" ? observation.amountValue : 0,
        sellVolume: observation.direction === "sell" ? observation.amountValue : 0,
        count: 1,
      });
      continue;
    }
    current.high = Math.max(current.high, observation.priceValue);
    current.low = Math.min(current.low, observation.priceValue);
    current.close = observation.priceValue;
    current.volume += observation.amountValue;
    current.count += 1;
    if (observation.direction === "buy") current.buyVolume += observation.amountValue;
    else current.sellVolume += observation.amountValue;
  }
  return [...buckets.values()].sort((a, b) => a.time - b.time);
}

export function percentChange(observations: QuoteObservation[]): number | null {
  if (observations.length < 2) return null;
  const first = observations[0].priceValue;
  const last = observations.at(-1)!.priceValue;
  return first > 0 ? (last - first) / first * 100 : null;
}

export function compactNumber(value: number): string {
  if (!Number.isFinite(value)) return "—";
  return new Intl.NumberFormat("en-US", { notation: "compact", maximumFractionDigits: 1 }).format(value);
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
