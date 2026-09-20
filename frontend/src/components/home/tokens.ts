export type TokenGroup = "fiat" | "stable" | "crypto";

export interface Token {
  symbol: string;
  name: string;
  nameRu: string;
  color: string;
  priceUsd: number;
  rail: string;
  group: TokenGroup;
  iconUrl?: string;
}

export const TOKENS: Token[] = [
  // Fiat
  { symbol: "EUR", name: "Euro", nameRu: "Евро", color: "#2556d2", priceUsd: 1.08, rail: "SEPA", group: "fiat" },
  { symbol: "USD", name: "US Dollar", nameRu: "Доллар США", color: "#2e7d32", priceUsd: 1, rail: "SWIFT", group: "fiat" },
  { symbol: "TRY", name: "Turkish Lira", nameRu: "Турецкая лира", color: "#c62828", priceUsd: 0.0292, rail: "SEPA-TRY", group: "fiat" },
  { symbol: "BRL", name: "Brazilian Real", nameRu: "Бразильский реал", color: "#219653", priceUsd: 0.19, rail: "PIX", group: "fiat" },
  { symbol: "RUB", name: "Russian Ruble", nameRu: "Российский рубль", color: "#7c4dff", priceUsd: 0.0108, rail: "SWIFT", group: "fiat" },
  { symbol: "KES", name: "Kenyan Shilling", nameRu: "Кенийский шиллинг", color: "#1aab00", priceUsd: 0.0078, rail: "M-PESA", group: "fiat" },
  // Stablecoins
  { symbol: "USDT", name: "Tether", nameRu: "Tether", color: "#26a17b", priceUsd: 1, rail: "TRC-20", group: "stable" },
  {
    symbol: "USDC",
    name: "USD Coin",
    nameRu: "USD Coin",
    color: "#2775ca",
    priceUsd: 1,
    rail: "ERC-20",
    group: "stable",
    iconUrl: "https://thumb.wikimedia.org/wikipedia/commons/thumb/4/4a/Circle_USDC_Logo.svg/1280px-Circle_USDC_Logo.svg.png?utm_source=en.wikipedia.org&utm_campaign=index&utm_content=thumbnail",
  },
  { symbol: "DAI", name: "Dai", nameRu: "Dai", color: "#f5ac37", priceUsd: 1, rail: "ERC-20", group: "stable" },
  // Crypto
  { symbol: "TON", name: "Toncoin", nameRu: "Toncoin", color: "#0098ea", priceUsd: 5.4, rail: "TON", group: "crypto" },
  {
    symbol: "ETH",
    name: "Ether",
    nameRu: "Ethereum",
    color: "#627eea",
    priceUsd: 3200,
    rail: "ERC-20",
    group: "crypto",
    iconUrl: "https://upload.wikimedia.org/wikipedia/commons/f/fd/Ethereum_Logo.png?utm_source=commons.wikimedia.org&utm_campaign=index&utm_content=original",
  },
  { symbol: "BTC", name: "Bitcoin", nameRu: "Bitcoin", color: "#f7931a", priceUsd: 96200, rail: "Bitcoin", group: "crypto" },
  { symbol: "SOL", name: "Solana", nameRu: "Solana", color: "#9945ff", priceUsd: 152, rail: "Solana", group: "crypto" },
  { symbol: "BNB", name: "BNB", nameRu: "BNB", color: "#f0b90b", priceUsd: 582, rail: "BEP-20", group: "crypto" },
];

export const GROUP_LABEL: Record<TokenGroup, string> = {
  fiat: "Fiat currencies",
  stable: "Stablecoins",
  crypto: "Cryptocurrency",
};

/** Look up a token by its currency symbol (case-insensitive). */
export function tokenForCurrency(symbol: string): Token | undefined {
  const upper = symbol.trim().toUpperCase();
  return TOKENS.find((token) => token.symbol === upper);
}

export function formatNumber(n: number, maxSignificant = 8): string {
  if (!Number.isFinite(n)) return "0";
  if (n === 0) return "0";
  const abs = Math.abs(n);
  const options: Intl.NumberFormatOptions = {
    useGrouping: abs >= 10000,
    maximumSignificantDigits: maxSignificant,
  };
  return new Intl.NumberFormat("en-US", options).format(n);
}

export function formatUsd(n: number): string {
  if (!Number.isFinite(n)) return "—";
  const abs = Math.abs(n);
  const fractionDigits = abs > 0 && abs < 1 ? 4 : 2;
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    minimumFractionDigits: 2,
    maximumFractionDigits: fractionDigits,
  }).format(n);
}

export function formatBalance(n: number): string {
  return new Intl.NumberFormat("en-US", { maximumFractionDigits: 2 }).format(n);
}

export const NETWORK_FEE_PERCENT = 0.8;

export function convert(sellAmount: number, sell: Token, buy: Token, feePercent: number = NETWORK_FEE_PERCENT): number {
  if (sellAmount <= 0) return 0;
  const gross = (sellAmount * sell.priceUsd) / buy.priceUsd;
  return gross * (1 - feePercent / 100);
}
