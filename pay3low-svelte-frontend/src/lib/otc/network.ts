export interface OtcEvmNetwork {
  name: string;
  label: string;
  chainId: number;
}

const EVM_NETWORKS: Record<number, { name: string; label: string }> = {
  1: { name: "ethereum", label: "Ethereum mainnet" },
  10: { name: "optimism", label: "Optimism" },
  56: { name: "bnb-smart-chain", label: "BNB Smart Chain" },
  100: { name: "gnosis", label: "Gnosis" },
  137: { name: "polygon-pos", label: "Polygon PoS" },
  8453: { name: "base", label: "Base" },
  42161: { name: "arbitrum-one", label: "Arbitrum One" },
  43114: { name: "avalanche-c", label: "Avalanche C-Chain" },
};

/**
 * The backend publishes the OTC chain id; signing against any other chain would
 * move funds the deal never observes, so unsupported networks stop the flow.
 */
export function evmNetworkFor(chainId: number | undefined): OtcEvmNetwork {
  const id = chainId ?? 1;
  const known = EVM_NETWORKS[id];
  if (!known) throw new Error(`OTC settlement network ${id} is not supported.`);
  return { ...known, chainId: id };
}

/** Chain labels from the backend are desk-side names; the wallet uses the network name. */
export function walletNetworkName(chain: string, network: OtcEvmNetwork): string {
  return chain === "ethereum" ? network.name : chain;
}
/** Peer settlement may run on its own Ethereum network; unset means the desk network. */
export function p2pEvmNetworkFor(config: { ethereum_chain_id?: number; p2p_ethereum_chain_id?: number }): OtcEvmNetwork {
  return evmNetworkFor(config.p2p_ethereum_chain_id ?? config.ethereum_chain_id);
}
