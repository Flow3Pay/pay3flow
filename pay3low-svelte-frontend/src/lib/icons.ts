const LOCAL_VENUE_ICONS = new Set(["binance", "bybit", "okx", "bitget", "rapira"]);
const LOCAL_ASSET_ICONS = new Set([
  "ada", "apt", "atom", "avax", "bch", "bnb", "btc", "dai", "doge", "dot", "eth", "fdusd",
  "link", "ltc", "matic", "near", "sol", "sui", "ton", "trx", "uni", "usdc", "usdt", "xrp",
]);

/** Returns a static asset served by this frontend. */
export function venueIcon(venue: string): string {
  const key = venue.toLowerCase();
  const extension = key === "binance" || key === "bybit" || key === "okx" ? "webp" : "svg";
  return LOCAL_VENUE_ICONS.has(key)
    ? `/icons/venues/${key}.${extension}`
    : "/icons/venues/generic.svg";
}

/** Returns a static asset served by this frontend. */
export function assetIcon(asset: string): string {
  const key = asset.toLowerCase();
  return LOCAL_ASSET_ICONS.has(key)
    ? `/icons/assets/${key}.webp`
    : "/icons/assets/generic.svg";
}
