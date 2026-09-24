const LOCAL_VENUE_ICONS = new Set(["binance", "bybit", "okx", "bitget", "rapira", "whitebird", "cifra-broker"]);
const LOCAL_ASSET_ICONS = new Set([
  "ada", "apt", "atom", "avax", "bch", "bnb", "btc", "dai", "doge", "dot", "eth", "fdusd",
  "link", "ltc", "matic", "near", "sol", "sui", "ton", "trx", "uni", "usdc", "usdt", "xrp",
]);

export const likeIcon = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAABa0lEQVR4AcyUv0tDMRDHfc5OLq6CiuDg4j9gF0dXUVBRBHEUxLXPuaCrKIJ0cXJwdLD+Wh1cbcFRBwdxFIf4SeCF6zVtU9pAy32Su3uX+yaBZnQk8W/4BIwxZWNMDtMxh+/pBDS9oWkOZWgQjzN3tGgBmi3RaRmkzcgg5EcJ0HyCxbeg7UsndOwFaDIHx3AB57AuiivCL9y/LMvei6Dd7AUouIJ92IYdqCJSglV8KUbo7M2NXQYpMB+onSI3CyEbCyV1Tgrob93iSU73AveKCrHfbD8CdgMLDIuKA+JrcNavgGsSGH6LXCqBp9QCdykFvmleA2cprqjGH9CKJBPw12MVkpzANi4YtMAz19P0hEiB10JVzA38E/iAGGu6HrtACqyQsK/mGfMprLGbR/jB34A6dLIqtUe6wAvwsQ6HsAt7YF9XV49vd2afBNvggaRmi5pN8i3mBVq+qAQNPiGHUoBLVe7DaAG/okfnHwAA//+PLiGuAAAABklEQVQDAFHSdTFISG9+AAAAAElFTkSuQmCC";
export const dislikeIcon = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAABbklEQVR4AcSUPUsDQRCGc4I/QLGxVFFL/QPiF4LYiY2CleBHlUprz1awshBFK1Gw0dIygr9AKwUVC3ttrS7PLOS4TDbZSdiQ8D53tzOz895u7q6v1OVf7wyyLFuFioeU2LB14d4V0OCKBvcw5+GQ2DM1y5yduN6AMziHY5hwCQ4NBiRT4lvQSmMkr6kdhFmub2EPduAA7sCpwYDoAlg0RFEZxkFruhaoM+BuJknMQDTVGdDVeveU2qQNFm3T7FW5AdszwLSurkCaiwk+8ZSvgJbRt4eepaLBkgRiUzT4j91c+hUN1gicwpPilXHHyg2SJHmHMswrpuj+Ax0pNwjM/muSfyH+CVoSdzGrgXxC3ITC4ZGVPkCF2CZcwCWcwDo4BQ14AUep7Act+dC5GCY3sAvbsA9vLsEhaECN7+WT/+qDXFAWgy/VRbZGnjYV9g+DBiz3l6kjcCQwXuFsVtBAOtH0G1JBxu1gMminoa6tAgAA//+gMzFGAAAABklEQVQDAO5fbjFNgnL7AAAAAElFTkSuQmCC";

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
  const extension = key === "binance" || key === "bybit" || key === "bitget" || key === "rapira" || key === "whitebird" || key === "cifra-broker" ? "png" : "svg";
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
