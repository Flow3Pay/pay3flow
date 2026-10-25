export interface OtcEvmNetwork {
  name: string;
  label: string;
  chainId: number;
}

const EVM_NETWORKS: Record<number, { name: string; label: string }> = {
  1: { name: "ethereum", label: "Ethereum mainnet" },
  11155111: { name: "sepolia", label: "Sepolia testnet" },
};

/**
 * The backend publishes the OTC chain id; signing against any other chain would
 * move funds the deal never observes, so an unknown chain stops the flow.
 */
export function evmNetworkFor(chainId: number | undefined): OtcEvmNetwork {
  const id = chainId ?? 1;
  const known = EVM_NETWORKS[id];
  if (!known) throw new Error(`Ethereum chain ${id} is not available in this wallet integration.`);
  return { ...known, chainId: id };
}

/** Chain labels from the backend are desk-side names; the wallet uses the network name. */
export function walletNetworkName(chain: string, network: OtcEvmNetwork): string {
  return chain === "ethereum" ? network.name : chain;
}
