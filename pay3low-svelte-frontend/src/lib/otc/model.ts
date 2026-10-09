/** Frontend OTC contracts. Prices and liquidity here are explicitly demo data. */
import type { CryptoNetwork } from "../networks";
export type OrderSide = "buy" | "sell";
export type OrderType = "limit" | "market";
export type ChartRange = "1D" | "7D" | "1M" | "1Y";
export interface OtcMarket {
  id: string;
  base: string;
  quote: string;
  name: string;
  price: number;
  priceDecimals: number;
  amountDecimals: number;
  tickSize: number;
  change: number;
  volume: number;
  icon: string;
}
export interface BookLevel { price: number; amount: number; total: number; depth: number }
export interface Candle { time: number; open: number; high: number; low: number; close: number; volume: number }
export interface OtcTrade { id: string; time: number; side: OrderSide; price: number; amount: number }
export interface OtcSnapshot {
  marketId: string;
  mode: "demo";
  bids: BookLevel[];
  asks: BookLevel[];
  trades: OtcTrade[];
}
export interface DemoOrder {
  id: string;
  marketId: string;
  side: OrderSide;
  type: OrderType;
  price: number;
  amount: number;
  createdAt: number;
  status: "open" | "cancelled" | "simulated";
  sendNetwork?: string;
  receiveNetwork?: string;
}
export const otcFallbackNetworks: CryptoNetwork[] = [
  { id: "everscale", name: "Everscale", currencies: ["EVER"] },
  { id: "bitcoin", name: "Bitcoin", currencies: ["BTC"] },
  { id: "ethereum", name: "Ethereum (ERC-20)", currencies: ["ETH", "USDT"] },
  { id: "solana", name: "Solana", currencies: ["SOL", "USDT"] },
  { id: "tron", name: "TRON (TRC-20)", currencies: ["USDT"] },
];
export const markets: OtcMarket[] = [
  { id: "EVER-USDT", base: "EVER", quote: "USDT", name: "Everscale", price: 0.01, priceDecimals: 5, amountDecimals: 6, tickSize: 0.00001, change: 2.84, volume: 12500, icon: "/icons/assets/ever.svg" },
  { id: "BTC-USDT", base: "BTC", quote: "USDT", name: "Bitcoin", price: 68240.5, priceDecimals: 2, amountDecimals: 6, tickSize: 10, change: 2.84, volume: 1248500, icon: "/icons/assets/btc.png" },
  { id: "ETH-USDT", base: "ETH", quote: "USDT", name: "Ethereum", price: 2648.72, priceDecimals: 2, amountDecimals: 5, tickSize: 0.5, change: 1.62, volume: 486320, icon: "/icons/assets/eth.png" },
  { id: "SOL-USDT", base: "SOL", quote: "USDT", name: "Solana", price: 148.36, priceDecimals: 2, amountDecimals: 4, tickSize: 0.05, change: 4.21, volume: 214850, icon: "/icons/assets/sol.webp" },
];
const anchorTime = Date.UTC(2026, 9, 9, 12);
export function marketFromHash(hash: string): OtcMarket {
  const params = new URLSearchParams(hash.split("?")[1] ?? "");
  return markets.find((market) => market.id === params.get("market")) ?? markets[0];
}
export function demoSnapshot(market: OtcMarket): OtcSnapshot {
  const levels = (side: OrderSide): BookLevel[] => {
    let cumulative = 0;
    return Array.from({ length: 9 }, (_, index) => {
      const price = Number((market.price + (side === "buy" ? -1 : 1) * market.tickSize * (index + 1)).toFixed(market.priceDecimals));
      const amount = Number(((3500 + ((index * 2371 + (side === "buy" ? 1030 : 610)) % 15500)) / market.price).toFixed(market.amountDecimals));
      cumulative += amount * price;
      return { price, amount, total: amount * price, depth: cumulative };
    });
  };
  return {
    marketId: market.id, mode: "demo", bids: levels("buy"), asks: levels("sell"),
    trades: Array.from({ length: 8 }, (_, index) => ({
      id: `${market.id}-${index}`, time: anchorTime - index * 73000,
      side: index % 3 === 0 ? "sell" : "buy",
      price: Number((market.price + Math.sin(index * 1.7) * market.tickSize * 3).toFixed(market.priceDecimals)),
      amount: Number(((600 + index * 780) / market.price).toFixed(market.amountDecimals)),
    })),
  };
}
export function demoCandles(market: OtcMarket, range: ChartRange): Candle[] {
  const days = { "1D": 1, "7D": 7, "1M": 30, "1Y": 365 }[range];
  const count = 64;
  let previous = market.price * .966;
  const samples = Array.from({ length: count }, (_, index) => {
    const trend = index / (count - 1);
    const wave = Math.sin(index * .34 + days * .17) * .009 + Math.sin(index * .91) * .0028;
    const close = market.price * (.968 + trend * .032 + wave + (trend > .65 ? Math.sin((trend - .65) * 14) * .012 : 0));
    const open = previous;
    previous = close;
    return {
      time: anchorTime - days * 86400000 * (1 - trend), open, close,
      high: Math.max(open, close) + market.price * (.0015 + (index % 5) * .00035),
      low: Math.min(open, close) - market.price * (.0013 + (index % 4) * .00042),
      volume: 1800 + (Math.sin(index * 2.13) + 1) * 1600 + (index % 11 === 0 ? 3400 : 0),
    };
  });
  const offset = market.price - samples[count - 1].close;
  return samples.map((candle) => ({ ...candle, open: candle.open + offset, close: candle.close + offset, high: candle.high + offset, low: candle.low + offset }));
}
export function formatPrice(value: number, market: OtcMarket): string {
  return value.toLocaleString("en-US", { minimumFractionDigits: market.priceDecimals, maximumFractionDigits: market.priceDecimals });
}
export function formatAmount(value: number, market: OtcMarket): string {
  return value.toLocaleString("en-US", { maximumFractionDigits: market.amountDecimals });
}
export function parsePositive(value: string): number | null {
  const normalized = value.trim().replace(",", ".");
  if (!/^\d+(?:\.\d+)?$/.test(normalized)) return null;
  const number = Number(normalized);
  return Number.isFinite(number) && number > 0 ? number : null;
}
export type OrderDraft = Pick<DemoOrder, "marketId" | "side" | "type" | "price" | "amount" | "sendNetwork" | "receiveNetwork"> & { total: number };
export type DraftResult = { draft: OrderDraft; error: null } | { draft: null; error: "input" | "price" | "precision" | "liquidity" | "range" };
/** Simulate the opposite book. Never submits transactions or contacts a wallet. */
export function prepareDemoOrder(market: OtcMarket, snapshot: OtcSnapshot, side: OrderSide, type: OrderType, input: string, limitPrice: string): DraftResult {
  const value = parsePositive(input);
  if (value === null) return { draft: null, error: "input" };
  if (value > 1e12) return { draft: null, error: "range" };
  let price = parsePositive(limitPrice), amount = 0, total = 0;
  if (type === "limit") {
    if (price === null || price > 1e12) return { draft: null, error: "price" };
    amount = side === "buy" ? value / price : value;
    total = amount * price;
  } else {
    let remaining = value;
    for (const level of side === "buy" ? snapshot.asks : snapshot.bids) {
      const take = side === "buy" ? Math.min(remaining / level.price, level.amount) : Math.min(remaining, level.amount);
      amount += take;
      total += take * level.price;
      remaining -= side === "buy" ? take * level.price : take;
      if (remaining <= value * 1e-10) break;
    }
    if (remaining > value * 1e-10) return { draft: null, error: "liquidity" };
    price = total / amount;
  }
  amount = Math.floor((amount + Number.EPSILON * amount) * 10 ** market.amountDecimals) / 10 ** market.amountDecimals;
  if (amount <= 0) return { draft: null, error: "precision" };
  total = amount * price;
  if (![amount, total, price].every(Number.isFinite) || total > 1e12) return { draft: null, error: "range" };
  return { draft: { marketId: market.id, side, type, price, amount, total }, error: null };
}
