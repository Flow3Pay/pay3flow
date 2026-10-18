import type { TrendEvent } from "./model";

/** Illustrative combinations and activity, separate from browser search history. */
export function demoTrendActivity(now = Date.now()): TrendEvent[] {
  const assets = [
    ["USDT", "tron"], ["BTC", "bitcoin"], ["ETH", "ethereum"], ["USDC", "ethereum"],
    ["SOL", "solana"], ["USDT", "ethereum"], ["GRAM", "ton"], ["USDT", "bsc"],
    ["BNB", "bsc"], ["LTC", "litecoin"], ["NEAR", "near"], ["EVER", "everscale"],
  ];
  const venues = ["bestchange", "bybit", "binance", "mexc", "near-intents", "okx"];
  const combinations = assets.flatMap(([asset, network], index) => ["", ...venues.slice(0, 4).map((_, venueIndex) => venues[(venueIndex + index) % venues.length])].map((provider) => ({ asset, network, provider })));
  return Array.from({ length: 30 * 8 }, (_, index) => ({
    at: now - (index + 0.5) * 3 * 3600000,
    blocks: combinations.filter((_, rank) => (index * 17 + rank * 13) % combinations.length < Math.max(4, combinations.length - 4 - rank)).map((block) => {
      const rank = combinations.indexOf(block);
      const source = rank % 3 === 0 ? "RUB" : rank % 3 === 1 ? "USDT" : "BYN";
      return { ...block, appearances: 1 + ((index * 7 + rank * 3) % Math.max(2, 19 - Math.floor(rank / 3))), source, target: block.asset === source ? "BTC" : block.asset };
    }),
  }));
}
