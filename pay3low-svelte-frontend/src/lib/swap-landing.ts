/** Human-readable asset names used by the swap landing pages and their copy. */
const assetNames: Record<string, string> = {
  BTC: "Bitcoin",
  ETH: "Ethereum",
  USDT: "Tether USD",
  USDC: "USD Coin",
  DAI: "Dai",
  BNB: "BNB",
  SOL: "Solana",
  TRX: "TRON",
  XRP: "XRP",
  ADA: "Cardano",
  DOGE: "Dogecoin",
  LTC: "Litecoin",
  DOT: "Polkadot",
  AVAX: "Avalanche",
  MATIC: "Polygon",
  BCH: "Bitcoin Cash",
  NEAR: "NEAR",
  SUI: "Sui",
  APT: "Aptos",
  ATOM: "Cosmos",
  LINK: "Chainlink",
  UNI: "Uniswap",
  GRAM: "Toncoin",
  EVER: "Everscale",
  RUB: "Russian ruble",
  KZT: "Kazakhstani tenge",
  UAH: "Ukrainian hryvnia",
  BYN: "Belarusian ruble",
  AMD: "Armenian dram",
  USD: "US dollar",
  EUR: "Euro",
  GEL: "Georgian lari",
  TRY: "Turkish lira",
  GBP: "British pound",
  UZS: "Uzbekistani sum",
  KGS: "Kyrgyzstani som",
  AZN: "Azerbaijani manat",
};

/** Display name for a ticker, falling back to the ticker itself. */
export function assetDisplayName(ticker: string): string {
  const key = ticker.trim().toUpperCase();
  return assetNames[key] ?? key;
}

/**
 * Directions worth a landing page of their own. Each pair is served at
 * `/swap/SOURCE/TARGET` and linked from the sitemap; the in-app amount,
 * provider and network filters stay in the query string.
 */
export const popularSwapPairs: ReadonlyArray<readonly [string, string]> = [
  ["USDT", "BTC"],
  ["BTC", "USDT"],
  ["USDT", "ETH"],
  ["ETH", "USDT"],
  ["USDT", "USDC"],
  ["USDC", "USDT"],
  ["BTC", "ETH"],
  ["ETH", "BTC"],
  ["USDT", "RUB"],
  ["RUB", "USDT"],
  ["USDT", "KZT"],
  ["KZT", "USDT"],
  ["USDT", "BYN"],
  ["BYN", "USDT"],
  ["USDT", "UAH"],
  ["UAH", "USDT"],
  ["USDT", "AMD"],
  ["AMD", "USDT"],
  ["USDT", "GRAM"],
  ["GRAM", "USDT"],
  ["BTC", "RUB"],
  ["RUB", "BTC"],
  ["ETH", "USDC"],
  ["USDC", "ETH"],
];

/** Whether a direction is one of the curated pages that should be indexed. */
export function isPopularSwapPair(source: string, target: string): boolean {
  return popularSwapPairs.some(([from, to]) => from === source && to === target);
}

/** Pairs that share a side with the current one, then popular filler directions. */
export function relatedSwapPairs(
  source: string,
  target: string,
  limit = 6,
): Array<readonly [string, string]> {
  const current = `${source}/${target}`;
  const related = popularSwapPairs.filter(
    ([from, to]) => `${from}/${to}` !== current && (from === source || to === target),
  );
  const seen = new Set(related.map(([from, to]) => `${from}/${to}`));
  for (const pair of popularSwapPairs) {
    if (related.length >= limit) break;
    const key = `${pair[0]}/${pair[1]}`;
    if (key === current || seen.has(key)) continue;
    seen.add(key);
    related.push(pair);
  }
  return related.slice(0, limit);
}
