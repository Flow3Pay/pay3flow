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
