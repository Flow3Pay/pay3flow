import { apiUrl } from "./api";
import { anonymousHeaders } from "$lib/anonymous-user";
import { walletToken, type WalletToken } from "./token-catalog-model";

export { balanceInputValue, walletToken } from "./token-catalog-model";
export type { WalletToken } from "./token-catalog-model";

const CACHE_TTL_MS = 10 * 60 * 1000;
let cached: { at: number; tokens: Promise<WalletToken[]> } | null = null;

async function loadTokens(): Promise<WalletToken[]> {
  const response = await fetch(apiUrl("/api/crypto-tokens"), {
    headers: anonymousHeaders({ Accept: "application/json" }),
  });
  if (!response.ok) throw new Error(`Token catalog request failed (${response.status})`);
  const tokens = (await response.json()) as WalletToken[];
  if (!Array.isArray(tokens)) throw new Error("Token catalog response is malformed");
  return tokens.filter((token) =>
    typeof token.network === "string"
    && typeof token.symbol === "string"
    && typeof token.decimals === "number"
    && (token.contract_address === null || typeof token.contract_address === "string"));
}

/** Cached token catalog. An empty catalog is retried on the next call. */
export function fetchWalletTokens(): Promise<WalletToken[]> {
  if (!cached || Date.now() - cached.at > CACHE_TTL_MS) {
    const at = Date.now();
    const tokens = loadTokens();
    cached = { at, tokens };
    tokens.then((list) => {
      if (!list.length && cached?.tokens === tokens) cached = null;
    }, () => {
      if (cached?.tokens === tokens) cached = null;
    });
  }
  return cached.tokens;
}

export async function resolveWalletToken(network: string, symbol: string): Promise<WalletToken | null> {
  try {
    return walletToken(await fetchWalletTokens(), network, symbol);
  } catch {
    // The Max button simply stays hidden while the catalog is unavailable.
    return null;
  }
}
