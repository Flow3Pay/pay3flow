/** Token identity a connected wallet can hold on one canonical network. */
export interface WalletToken {
  network: string;
  symbol: string;
  decimals: number;
  /** Contract or mint address; null marks the network's native coin. */
  contract_address: string | null;
}

/**
 * Find the wallet token for a network and ticker. A catalog entry with a real
 * contract wins over a native placeholder that happens to share the ticker.
 */
export function walletToken(
  tokens: WalletToken[],
  network: string,
  symbol: string,
): WalletToken | null {
  const wantedNetwork = network.trim().toLowerCase();
  const wantedSymbol = symbol.trim().toUpperCase();
  if (!wantedNetwork || !wantedSymbol) return null;
  const matches = tokens.filter((token) =>
    token.network.toLowerCase() === wantedNetwork
    && token.symbol.trim().toUpperCase() === wantedSymbol);
  return matches.find((token) => Boolean(token.contract_address)) ?? matches[0] ?? null;
}

/** Exact balance as an input value: no exponent, no trailing zeros. */
export function balanceInputValue(balance: string | null, decimals: number): string | null {
  if (!balance) return null;
  const [whole, fraction = ""] = balance.split(".");
  const trimmed = fraction.slice(0, Math.max(0, decimals)).replace(/0+$/, "");
  const value = trimmed ? `${whole}.${trimmed}` : whole;
  return /^[0-9]+(\.[0-9]+)?$/.test(value) && Number(value) > 0 ? value : null;
}
