import { API_BASE_URL } from "./api";

export interface CryptoNetwork {
  id: string;
  name: string;
  currencies: string[];
}

export const FALLBACK_NETWORK: CryptoNetwork = {
  id: "ethereum",
  name: "Ethereum (ERC-20)",
  currencies: ["ETH", "USDT", "USDC", "BNB", "DAI", "FDUSD", "LINK", "MATIC", "UNI"],
};

export async function fetchNetworks(currency?: string): Promise<CryptoNetwork[]> {
  const url = new URL("/api/networks", API_BASE_URL);
  if (currency) url.searchParams.set("currency", currency);

  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Network catalog request failed (${response.status})`);
  }

  return (await response.json()) as CryptoNetwork[];
}
