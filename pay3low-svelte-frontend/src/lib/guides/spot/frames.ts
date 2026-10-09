import type { TutorialFrame, TutorialStep } from "$lib/route-tutorial";

type Copy = (key: string, params?: Record<string, string | number>) => string;
export const spotGuideLinks: Record<string, { label: string; url: string } | undefined> = {
  binance: { label: "Binance Spot", url: "https://www.binance.com/en/support/faq/detail/12cba755d6334ad98ced0b66ddde66ec" },
  bybit: { label: "Bybit Spot", url: "https://www.bybit.com/en/help-center/article/How-to-Get-Started-with-Spot-Trading" },
  mexc: { label: "MEXC Spot", url: "https://www.mexc.com/support/article/how-to-use-market-orders-in-spot-trading-332255423024245760" },
  bitget: { label: "Bitget Spot", url: "https://www.bitget.com/support/articles/12560603820568" },
  okx: { label: "OKX Spot", url: "https://www.okx.com/trade-spot/btc-usdt" },
  whitebird: { label: "Whitebird Spot", url: "https://whitebird.io/spot/BTC/USDT" },
};
export const spotProfiles: Record<string, { buyField: string; sellField: string; location: string; layout: "below-chart" | "right-panel"; history: string } | undefined> = {
  binance: { buyField: "Total", sellField: "Amount", location: "Spot panel below the chart", layout: "below-chart", history: "Trade History / Order History" },
  bybit: { buyField: "Order by Value", sellField: "Order by Qty", location: "Spot panel on the right", layout: "right-panel", history: "Trade History / Order History" },
  mexc: { buyField: "Total", sellField: "Amount", location: "Spot panel on the right", layout: "right-panel", history: "Trade History / Order History" },
  bitget: { buyField: "Total", sellField: "Quantity", location: "Spot panel on the right", layout: "right-panel", history: "Order History / Order details" },
  okx: { buyField: "Total", sellField: "Amount", location: "Spot panel on the right", layout: "right-panel", history: "Order History / Trade History" },
  whitebird: { buyField: "Total", sellField: "Quantity", location: "Trading Panel on the right", layout: "right-panel", history: "Order History / Trade History" },
};
export interface SpotTrade { base: string; quote: string; side: "buy" | "sell"; label: string }
/** Infer direction from the actual market symbol, never from a stablecoin list. */
export function spotTrade(symbol: string | undefined, from: string, to: string): SpotTrade | null {
  const normalized = symbol?.replace(/[^a-z0-9]/gi, "").toUpperCase();
  const source = from.toUpperCase(), target = to.toUpperCase();
  if (!normalized || source === target) return null;
  const side = normalized === `${source}${target}` ? "sell" : normalized === `${target}${source}` ? "buy" : null;
  if (!side) return null;
  const base = side === "buy" ? target : source, quote = side === "buy" ? source : target;
  return { base, quote, side, label: `${base} / ${quote}` };
}
export function buildSpotFrames(step: Pick<TutorialStep, "provider" | "venue" | "pair" | "from" | "to" | "amount">, copy: Copy): TutorialFrame[] | null {
  const trade = spotTrade(step.pair, step.from, step.to);
  if (!trade || !spotGuideLinks[step.provider.toLowerCase()]) return null;
  const provider = step.provider.toLowerCase();
  const profile = spotProfiles[provider]!;
  const field = trade.side === "buy" ? profile.buyField : profile.sellField;
  const action = copy(trade.side === "buy" ? "Buy {asset}" : "Sell {asset}", { asset: trade.base });
  const account = provider === "bybit"
    ? "Open Spot on Bybit. Sign in and make sure {from} is available in your Unified Trading Account. Transfer it from Funding if needed, or deposit using the exact supported network."
    : provider === "whitebird"
    ? "Open Spot on Whitebird and sign in to unlock the Trading Panel. Check your available {from} balance and the deposit or internal transfer options in your account. Use the exact supported deposit network."
    : "Open Trade → Spot on {venue}. Sign in and make sure {from} is available in your Spot account. Transfer it internally if needed, or deposit using the exact supported network.";
  return [
    { kind: "open", title: copy("Open {venue} Spot", { venue: step.venue }), text: copy(account, { from: step.from, venue: step.venue }) },
    { kind: "verify", title: copy("Select {pair}", { pair: trade.label }), text: copy(trade.side === "buy"
      ? "Find {pair} in the market search. In the {location}, select Buy: you spend {quote} to receive {base}. Use ordinary Spot with borrowing disabled."
      : "Find {pair} in the market search. In the {location}, select Sell: you spend {base} to receive {quote}. Use ordinary Spot with borrowing disabled.", { ...trade, pair: trade.label, location: copy(profile.location) }) },
    { kind: "review", title: copy("Choose the order type and amount"), text: copy("Select Market in the {location}. Enter {amount} in {field}, denominated in {from}. For Limit, also set your price and review the quantity and total. A limit order may remain unfilled.", { amount: step.amount ?? step.from, from: step.from, field: copy(field), location: copy(profile.location) }) },
    { kind: "act", title: copy("Review and press {action}", { action }), text: copy("Check the pair, direction, available balance, minimum order, fees and expected slippage on {venue}. Press {action} only after reviewing the actual order details and any confirmation window.", { venue: step.venue, action }) },
    { kind: "receive", title: copy("Check Trade History and balance"), text: copy("Open {history} on {venue}. Check the filled quantity, average price and fee, then verify your actual {to} balance. Continue only with the amount received; the route estimate is not a completed trade.", { history: profile.history, venue: step.venue, to: step.to }) },
  ];
}

export const spotGuideVenues = ["binance", "bybit", "mexc", "whitebird", "bitget", "okx"];

export function isSpotStep(step: Pick<TutorialStep, "kind" | "pair" | "provider" | "from" | "to">): boolean {
  return step.kind === "swap" && Boolean(spotMarket(step)) && spotGuideVenues.includes(step.provider.toLowerCase());
}

/** The order side belongs to the market's base asset, not the route's input asset. */
export function spotMarket(step: Pick<TutorialStep, "pair" | "from" | "to">) {
  const trade = spotTrade(step.pair, step.from, step.to);
  return trade ? { base: trade.base, quote: trade.quote, buying: trade.side === "buy" } : undefined;
}
