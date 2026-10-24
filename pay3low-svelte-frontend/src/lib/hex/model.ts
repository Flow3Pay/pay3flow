export const CONTRACT = "orderbook.fi.tg";
export const GNK = "nep245:v2_1.omni.hot.tg:4444119_wyixUKCL";
export const USDT = "nep141:usdt.tether-token.near";
export const PAIR = { base: GNK, quote: USDT };
export type Side = "buy" | "sell";
export type OrderType = "market" | "limit";
export interface Config { execution_enabled: boolean; contract: string; base_token: string; quote_token: string }
export interface RawLevel { price: string; order_balance: string; hash: string; owner?: string }
export interface Order { hash: string; order_balance: string; order: { side: Side; price: string; maker: string; created_ts: string; token_pair: typeof PAIR } }
export interface Account { account: string; orders: Order[]; pending: Record<string, string>; balances: string[] }
export interface HistoryEntry { type: string; timestamp: number; amount: number; amount_to: number | null; price: number | null; near_trx: string; intents_id: string; token_id: string; token_to: string; involved_account_id?: string | null }
export interface Snapshot { book: { bids: RawLevel[]; asks: RawLevel[] }; fees: { maker_fee_collectors: { fee: number; collector: string }[]; taker_fee_collectors: { fee: number; collector: string }[] }; fetched_at: string }
export interface Draft { side: Side; order_type: OrderType; amount: string; price?: string }
export interface Quote { result: { book_fill_result: { taker_receive_total: string; taker_unspent: string; taker_fee_quote: string }; taker_remainder?: { new_order?: string; refund?: string } | null }; expires_at: string }
export interface DisplayLevel { price: string; amount: string; total: string; depth: number }

/** Decimal input is exact and bounded before any BigInt conversion. */
export function units(value: string, decimals: number): bigint {
  const text = value.trim().replace(",", ".");
  if (text.length > 64 || !/^\d+(?:\.\d+)?$/.test(text)) throw new Error("Enter a positive decimal amount");
  const [whole, fraction = ""] = text.split(".");
  if (fraction.length > decimals) throw new Error(`Maximum ${decimals} decimal places`);
  const result = BigInt(whole + fraction.padEnd(decimals, "0"));
  if (result <= 0n || result > (1n << 128n) - 1n) throw new Error("Amount is outside the supported range");
  return result;
}
export function decimal(value: string | bigint, decimals: number): string {
  const n = BigInt(value), scale = 10n ** BigInt(decimals);
  const fraction = (n % scale).toString().padStart(decimals, "0").replace(/0+$/, "");
  return `${n / scale}${fraction ? `.${fraction}` : ""}`;
}
// Contract prices have up to 24 decimals and account for 9/6 token decimals.
const PRICE_SCALE = 10n ** 24n;
export function contractPrice(price: string): string { return decimal(units(price, 21), 24); }
export function humanPrice(price: string): string { return decimal(units(price, 24), 21); }
export function makeDraft(side: Side, order_type: OrderType, amount: string, price: string): Draft {
  return { side, order_type, amount: units(amount, side === "buy" ? 6 : 9).toString(),
    ...(order_type === "limit" ? { price: contractPrice(price) } : {}) };
}
export function levels(raw: RawLevel[], side: Side): DisplayLevel[] {
  const groups = new Map<string, { price: bigint; base: bigint; quote: bigint }>();
  for (const entry of raw) {
    const price = units(entry.price, 24), balance = BigInt(entry.order_balance);
    const key = price.toString(), group = groups.get(key) ?? { price, base: 0n, quote: 0n };
    group.base += side === "sell" ? balance : balance * PRICE_SCALE / price;
    group.quote += side === "buy" ? balance : balance * price / PRICE_SCALE;
    groups.set(key, group);
  }
  let cumulative = 0;
  return [...groups.values()].sort((a,b) => a.price === b.price ? 0 : (a.price < b.price ? -1 : 1) * (side === "sell" ? 1 : -1))
    .map(group => { const total = decimal(group.quote, 6); cumulative += Number(total);
      return { price: decimal(group.price, 21), amount: decimal(group.base, 9), total, depth: cumulative }; });
}
export function minimumOutput(expected: string, percent: string): string {
  // Percent with two decimal places: 0.5% = 50 / 10,000.
  const bps = /^0(?:[.,]0{1,2})?$/.test(percent.trim()) ? 0n : units(percent, 2);
  if (bps >= 10_000n) throw new Error("Slippage must be at least 0 and less than 100%");
  const minimum = BigInt(expected) * (10_000n - bps) / 10_000n;
  if (minimum <= 0n) throw new Error("No executable output for this amount");
  return minimum.toString();
}
export function refundedInput(quote: Quote): string {
  const remainder = quote.result.taker_remainder;
  if (remainder?.refund !== undefined) return remainder.refund;
  const unspent = BigInt(quote.result.book_fill_result.taker_unspent), resting = BigInt(remainder?.new_order ?? "0");
  // HEX v1 counts resting input as taker_unspent, although it remains locked.
  return (unspent > resting ? unspent - resting : 0n).toString();
}
export function orderMessage(draft: Draft, quote: Quote, slippage: string) {
  return { token_pair: PAIR, order_type: draft.order_type, refs: [],
    ...(draft.order_type === "limit" ? { price: draft.price } : { min_amount_out: minimumOutput(quote.result.book_fill_result.taker_receive_total, slippage) }) };
}
export function cancelMessage(hash: string) {
  if (!/^[a-f0-9]{64}$/.test(hash)) throw new Error("Invalid order hash");
  return { action: "close_order", data: { hash, token_pair: PAIR } };
}
export function settledTrades(history: HistoryEntry[]): HistoryEntry[] {
  return history.filter(t => (t.type === "buy" || t.type === "sell") && Number.isFinite(t.price) && Number(t.price) > 0 && Number.isFinite(t.amount) && t.amount > 0 && Number.isFinite(t.timestamp))
    .sort((a,b) => a.timestamp - b.timestamp);
}
export function chartPoints(history: HistoryEntry[]): string {
  const trades = settledTrades(history), prices = trades.map(t => t.price!);
  if (!prices.length) return "";
  const min = Math.min(...prices), span = Math.max(...prices) - min || min * .02;
  return trades.map((t,i) => `${20 + i / Math.max(1,trades.length-1)*560},${220-(t.price!-min)/span*180}`).join(" ");
}
export function isQuoteCurrent(quote: Quote, now = Date.now()): boolean {
  return Number.isFinite(Date.parse(quote.expires_at)) && Date.parse(quote.expires_at) > now;
}
