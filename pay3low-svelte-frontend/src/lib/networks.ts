import { apiUrl } from "./api";
import { anonymousHeaders } from "$lib/anonymous-user";

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

const NATIVE_NETWORKS: Record<string, string> = {
  BTC: "bitcoin", ETH: "ethereum", BNB: "bnb-smart-chain", SOL: "solana",
  TRX: "tron", TON: "ton", XRP: "xrpl", ADA: "cardano",
  DOT: "polkadot", AVAX: "avalanche-c", MATIC: "polygon-pos",
  BCH: "bitcoin-cash", DOGE: "dogecoin", LTC: "litecoin",
  NEAR: "near", APT: "aptos", ATOM: "cosmos-hub", SUI: "sui",
};

export function preferredNetwork(currency: string, networks: CryptoNetwork[]): CryptoNetwork | undefined {
  const matching = networks.filter((network) => network.currencies.includes(currency));
  return matching.find((network) => network.id === NATIVE_NETWORKS[currency])
    ?? matching.find((network) => network.id === "ethereum")
    ?? matching[0];
}

export async function fetchNetworks(currency?: string): Promise<CryptoNetwork[]> {
  const url = apiUrl("/api/networks");
  if (currency) url.searchParams.set("currency", currency);

  const response = await fetch(url, { headers: anonymousHeaders({ Accept: "application/json" }) });
  if (!response.ok) {
    throw new Error(`Network catalog request failed (${response.status})`);
  }

  return (await response.json()) as CryptoNetwork[];
}
