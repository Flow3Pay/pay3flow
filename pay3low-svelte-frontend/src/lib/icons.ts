const LOCAL_VENUE_ICONS = new Set(["binance", "bybit", "okx", "bitget", "rapira", "whitebird"]);
const LOCAL_ASSET_ICONS = new Set([
  "ada", "apt", "atom", "avax", "bch", "bnb", "btc", "dai", "doge", "dot", "eth", "fdusd",
  "link", "ltc", "matic", "near", "sol", "sui", "ton", "trx", "uni", "usdc", "usdt", "xrp",
]);

/** Network marks reuse the native coin mark where the network and coin are synonymous. */
const NETWORK_ASSETS: Array<[RegExp, string]> = [
  [/bitcoin cash/i, "bch"],
  [/bitcoin/i, "btc"],
  [/ethereum|arbitrum|optimism|base/i, "eth"],
  [/bnb smart chain|bep-20/i, "bnb"],
  [/solana/i, "sol"],
  [/tron|trc-20/i, "trx"],
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
  const key = venue.toLowerCase();
  const extension = key === "binance" || key === "bybit" || key === "bitget" || key === "rapira" || key === "whitebird" ? "png" : "svg";
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

/** Returns the native-asset mark used to identify a blockchain network. */
export function networkIcon(network?: string): string {
  const match = network ? NETWORK_ASSETS.find(([pattern]) => pattern.test(network)) : undefined;
  return assetIcon(match?.[1] ?? "");
}
