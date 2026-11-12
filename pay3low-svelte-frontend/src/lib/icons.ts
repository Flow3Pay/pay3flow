const LOCAL_VENUE_ICONS = new Set(["solunex", "binance", "bybit", "okx", "bitget", "mexc", "rapira", "whitebird", "cifra-broker", "cow-swap", "bestchange", "bitcoin-center", "bncex", "dzengi", "id-pay", "skylabs", "papa-change", "crypto-office", "symbiosis", "near-intents", "gate", "barter", "1inch", "0x", "lifi", "nordstern", "velora", "enso", "bebop", "kyberswap", "cbr", "rate-am", "pay3flow-otc"]);
const PNG_VENUE_ICONS = new Set(["solunex", "binance", "bybit", "bitget", "mexc", "rapira", "whitebird", "cifra-broker", "papa-change", "symbiosis", "near-intents", "gate", "1inch", "0x", "lifi", "velora", "enso", "bebop", "kyberswap", "cbr"]);
const JPG_VENUE_ICONS = new Set(["nordstern"]);
const VENUE_ICON_ALIASES: Record<string, string> = { "1ich": "1inch", lighing: "lifi", verola: "velora", "rate.am": "rate-am" };
const LOCAL_ASSET_ICONS = new Set([
  "ada", "apt", "atom", "avax", "bch", "bnb", "btc", "dai", "doge", "dot", "eth", "fdusd",
  "link", "ltc", "matic", "near", "sol", "sui", "ton", "trx", "uni", "usdc", "usdt", "xrp",
]);
const PNG_ASSET_ICONS = new Set(["10set", "acnon", "ab", "ai", "aave", "aixbt", "aevo", "cro", "sei"]);
const LOCAL_FIAT_ICONS = new Set(["amd", "byn", "kzt", "rub", "uah", "usd"]);

export const likeIcon = "/icons/ui/like.svg";
export const dislikeIcon = "/icons/ui/dislike.svg";
export const warningIcon = "/icons/ui/warning.svg";

/** Network marks reuse the native coin mark where the network and coin are synonymous. */
const NETWORK_ASSETS: Array<[RegExp, string]> = [
  [/bitcoin cash/i, "bch"],
  [/bitcoin/i, "btc"],
  [/ethereum|arbitrum|optimism|base/i, "eth"],
  [/(?:bnb|binance)[- ]smart[- ]chain|\bbsc\b|\bbep[- ]?20\b/i, "bnb"],
  [/solana/i, "sol"],
  [/^cronos$/i, "cro"],
  [/^sei$/i, "sei"],
  [/tron|trc-20/i, "trx"],
  [/everscale|^ever$/i, "ever"],
  [/ton/i, "ton"],
  [/dogecoin/i, "doge"],
  [/litecoin/i, "ltc"],
  [/xrp ledger|xrpl/i, "xrp"],
  [/cardano/i, "ada"],
  [/polkadot/i, "dot"],
  [/avalanche/i, "avax"],
  [/polygon/i, "matic"],
  [/near/i, "near"],
  [/aptos/i, "apt"],
  [/cosmos/i, "atom"],
  [/sui/i, "sui"],
];

/** Returns a static asset served by this frontend. */
export function venueIcon(venue: string): string {
  const rawKey = venue.toLowerCase();
  const key = VENUE_ICON_ALIASES[rawKey] ?? rawKey;
  const extension = PNG_VENUE_ICONS.has(key) ? "png" : JPG_VENUE_ICONS.has(key) ? "jpg" : "svg";
  const filename = key === "cow-swap" ? "cow-swap-favicon.svg" : `${key}.${extension}`;
  return LOCAL_VENUE_ICONS.has(key)
    ? `/icons/venues/${filename}`
    : "/icons/venues/generic.svg";
}

/** Returns a static asset served by this frontend. */
export function assetIcon(asset: string): string {
  const key = asset.toLowerCase();
  if (key === "gnk") return "/icons/assets/gnk.jpg";
  if (key === "any") return "/icons/assets/any.svg";
  if (key === "gram") return "/icons/assets/gram.png";
  if (key === "near") return "/icons/assets/near-investx.png";
  if (key === "ever") return "/icons/assets/ever.svg";
  if (PNG_ASSET_ICONS.has(key)) return `/icons/assets/${key}.png`;
  if (LOCAL_FIAT_ICONS.has(key)) return `/icons/assets/${key}.svg`;
  return LOCAL_ASSET_ICONS.has(key)
    ? `/icons/assets/${key}.webp`
    : "/icons/assets/generic.svg";
}

/** Returns the native-asset mark used to identify a blockchain network. */
export function networkIcon(network?: string): string {
  const match = network ? NETWORK_ASSETS.find(([pattern]) => pattern.test(network)) : undefined;
  return assetIcon(match?.[1] ?? "");
}
