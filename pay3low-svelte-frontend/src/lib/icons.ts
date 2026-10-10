const LOCAL_VENUE_ICONS = new Set(["binance", "bybit", "okx", "bitget", "mexc", "rapira", "whitebird", "cifra-broker", "cow-swap", "bestchange", "bitcoin-center", "bncex", "dzengi", "id-pay", "skylabs", "papa-change", "symbiosis", "1inch", "0x", "lifi", "nordstern", "velora", "enso", "bebop", "kyberswap"]);
const PNG_VENUE_ICONS = new Set(["binance", "bybit", "bitget", "mexc", "rapira", "whitebird", "cifra-broker", "papa-change", "symbiosis", "1inch", "0x", "lifi", "velora", "enso", "bebop", "kyberswap"]);
const JPG_VENUE_ICONS = new Set(["nordstern"]);
const VENUE_ICON_ALIASES: Record<string, string> = { "1ich": "1inch", lighing: "lifi", verola: "velora" };
const LOCAL_ASSET_ICONS = new Set([
  "ada", "apt", "atom", "avax", "bch", "bnb", "btc", "dai", "doge", "dot", "eth", "fdusd",
  "link", "ltc", "matic", "near", "sol", "sui", "ton", "trx", "uni", "usdc", "usdt", "xrp",
]);
const LOCAL_FIAT_ICONS = new Set(["amd", "byn", "kzt", "rub", "uah", "usd"]);

export const likeIcon = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAABa0lEQVR4AcyUv0tDMRDHfc5OLq6CiuDg4j9gF0dXUVBRBHEUxLXPuaCrKIJ0cXJwdLD+Wh1cbcFRBwdxFIf4SeCF6zVtU9pAy32Su3uX+yaBZnQk8W/4BIwxZWNMDtMxh+/pBDS9oWkOZWgQjzN3tGgBmi3RaRmkzcgg5EcJ0HyCxbeg7UsndOwFaDIHx3AB57AuiivCL9y/LMvei6Dd7AUouIJ92IYdqCJSglV8KUbo7M2NXQYpMB+onSI3CyEbCyV1Tgrob93iSU73AveKCrHfbD8CdgMLDIuKA+JrcNavgGsSGH6LXCqBp9QCdykFvmleA2cprqjGH9CKJBPw12MVkpzANi4YtMAz19P0hEiB10JVzA38E/iAGGu6HrtACqyQsK/mGfMprLGbR/jB34A6dLIqtUe6wAvwsQ6HsAt7YF9XV49vd2afBNvggaRmi5pN8i3mBVq+qAQNPiGHUoBLVe7DaAG/okfnHwAA//+PLiGuAAAABklEQVQDAFHSdTFISG9+AAAAAElFTkSuQmCC";
export const dislikeIcon = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAABbklEQVR4AcSUPUsDQRCGc4I/QLGxVFFL/QPiF4LYiY2CleBHlUprz1awshBFK1Gw0dIygr9AKwUVC3ttrS7PLOS4TDbZSdiQ8D53tzOz895u7q6v1OVf7wyyLFuFioeU2LB14d4V0OCKBvcw5+GQ2DM1y5yduN6AMziHY5hwCQ4NBiRT4lvQSmMkr6kdhFmub2EPduAA7sCpwYDoAlg0RFEZxkFruhaoM+BuJknMQDTVGdDVeveU2qQNFm3T7FW5AdszwLSurkCaiwk+8ZSvgJbRt4eepaLBkgRiUzT4j91c+hUN1gicwpPilXHHyg2SJHmHMswrpuj+Ax0pNwjM/muSfyH+CVoSdzGrgXxC3ITC4ZGVPkCF2CZcwCWcwDo4BQ14AUep7Act+dC5GCY3sAvbsA9vLsEhaECN7+WT/+qDXFAWgy/VRbZGnjYV9g+DBiz3l6kjcCQwXuFsVtBAOtH0G1JBxu1gMminoa6tAgAA//+gMzFGAAAABklEQVQDAO5fbjFNgnL7AAAAAElFTkSuQmCC";
export const warningIcon = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAAB80lEQVR4AeyVPUoDURRGJ1Z2Vi7BQtTen0IrFaxEVFAEC0HdgI2WuoeIhZWCkYBtUtkkjVgpGMgSkgXYxXOSSZhhMpMJaKfck3ff/fmuefieE8Ef/+Qa0Ol0puEY7kL0p/P8biMHIHiKUA3uYSlEvxbmCKVb6gCa16FCaxE+YKVQKMyLPhgrWgPr7IdaYgDFM6Co4h7DIaI7UO8r6MMO+0OwpmIPzLCPWWwABbtkPY591itYRuiRdaiFuWWS1trjsalBqGeDAYhvEirBKyh8g8A3fqZZAzcUOcjeUqhFKAgGA9itQpviPfjCTxiNL5JIELAH9nDboBZLfMAikU/IsimSwpJqaqjVLYh+g27gtz/+B4w80XGP6A1FYclnYw3gz/BC8kn3qsYawB04kV5rvs/ogCotc5Bl2ySFJdXUUKtbEB3wTsR336s+i58wjmdLEgkCfLNZ8Knx8VOLaOQm0+hUr/oamTrFlzCJn2nWwCVFvrb2+tSoRSgywB1DnllX4AmuwUEHrEMNYXMKW2uP/zPUGNRHj6gbZEgTzthsQAseECqDryXbINCHMpsHsGbDHmiyj1liQD9LcRUc4rAF4r71DYQb+mDszBoYHAnxmKUO6FfRfIvvsZ2zeslE3+MwRzjdRg6wlSEtKMJRiL5HYzqTHwAAAP//eL4BeAAAAAZJREFUAwAimsoxLBYOAwAAAABJRU5ErkJggg==";

export const swapIcon = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAABN0lEQVR4AdyUPYrCQBSAk90tdquFvcD6U1tpZaE38A6CtaWH8AgieA1rQQW1tvOn0iNoI/F74MhkzMSYiQjK+5yXF+Z9TJiZD+/Jv9cLgiDIQdFcKLUylMy6+ZxkBX0mDcCMIYU5kgKjNZIIbJM3vPBhhOSfMTJcBEc6LuALxjaJi4C+3oG/KijJzedyFXi+768RiITBm7CSkMRZIF0vkjr5CbpwjUwE0g3JirECbbhGZgLpiGQPO8kVmQpUU318MwFb7A9+9SW65uYn6tFwiSS0l6mlDlPQodMnxN4vvE8cIQFbTJ1KdfStl1hSQ0ggk0wJtR9IHTcC6aRJ5FFO57ckaYgUSKOLRO6XgOc8pAqrQLohkfulRt4AM5oUWhAbsQKZiWQGU8l1qK1gq9ei8ruCqEmP1M4AAAD//7Ma7NIAAAAGSURBVAMASrFMMYtFRyMAAAAASUVORK5CYII=";

/** Network marks reuse the native coin mark where the network and coin are synonymous. */
const NETWORK_ASSETS: Array<[RegExp, string]> = [
  [/bitcoin cash/i, "bch"],
  [/bitcoin/i, "btc"],
  [/ethereum|arbitrum|optimism|base/i, "eth"],
  [/(?:bnb|binance)[- ]smart[- ]chain|\bbsc\b|\bbep[- ]?20\b/i, "bnb"],
  [/solana/i, "sol"],
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
  if (key === "near-intents") return "/icons/assets/near.webp";
  const extension = PNG_VENUE_ICONS.has(key) ? "png" : JPG_VENUE_ICONS.has(key) ? "jpg" : "svg";
  const filename = key === "cow-swap" ? "cow-swap-favicon.svg" : `${key}.${extension}`;
  return LOCAL_VENUE_ICONS.has(key)
    ? `/icons/venues/${filename}`
    : "/icons/venues/generic.svg";
}

/** Returns a static asset served by this frontend. */
export function assetIcon(asset: string): string {
  const key = asset.toLowerCase();
  if (key === "ever") return "/icons/assets/ever.svg";
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
