import type { RouteCandidate } from "./exchange";
import type { TutorialStep } from "./route-tutorial";

export interface SwapInput {
  routeId: string;
  provider: string;
  fromAsset: string;
  toAsset: string;
  amount: string;
  fromTokenAddress?: string;
  toTokenAddress?: string;
}

export interface SwapServiceState {
  address?: string;
  output?: string;
  ready?: boolean;
  loading?: boolean;
  error?: string;
  depositAddress?: string;
  depositMemo?: string;
  status?: string;
}

export interface PreparedSwap {
  output: string;
  expiresAt: string;
  submit(): Promise<{ reference: string; approval?: boolean }>;
  status(reference: string): Promise<string>;
}

export interface SwapService {
  connect(): Promise<void>;
  setRecipient(address: string): void;
  prepare(): Promise<PreparedSwap>;
  setExternalDeposit?(refundAddress: string): void;
  recover?(): Promise<{ prepared: PreparedSwap; reference?: string; recipient: string; depositAddress?: string; depositMemo?: string } | null>;
  destroy(): void;
}

/** A composed route executes the selected leg, never the whole route's fiat amount. */
export function swapInput(route: RouteCandidate, step: TutorialStep, networks: Record<string, string>): SwapInput {
  const networkId = (name: string | undefined) => Object.keys(networks).find(id => networks[id] === name) ?? name;
  const paths = route.route_path?.filter(asset => asset.includes("@")) ?? [];
  const fromNetwork = networkId(step.network) ?? route.source_network ?? route.entry_network;
  const toNetwork = networkId(step.targetNetwork) ?? route.target_network ?? fromNetwork;
  const fromAsset = step.fromAsset ?? (step.execution ? route.execution?.from_asset : undefined) ?? paths[0] ?? `${step.from}@${fromNetwork ?? ""}`;
  const toAsset = step.toAsset ?? (step.execution ? route.execution?.to_asset : undefined) ?? paths.at(-1) ?? `${step.to}@${toNetwork ?? ""}`;
  const formattedAmount = step.amount?.endsWith(` ${step.from}`) ? step.amount.slice(0, -step.from.length - 1).replaceAll(",", "") : undefined;
  const amount = step.inputAmount ?? (step.execution ? route.execution?.input_amount : undefined) ?? formattedAmount ?? (route.entry_offer_snapshot ? route.acquired_asset_amount : route.source_amount) ?? "";
  const cowTokens = step.provider === "cow-swap" ? step.url?.match(/#\/\d+\/swap\/([^/?#]+)\/([^/?#]+)/) : null;
  const contract = (value: string | undefined) => value && /^0x[0-9a-f]{40}$/i.test(value) ? value : undefined;
  return { routeId: `${route.route_id}:${step.id}`, provider: step.provider, fromAsset, toAsset, amount,
    fromTokenAddress: contract(cowTokens?.[1]), toTokenAddress: contract(cowTokens?.[2]) };
}

export async function createSwapService(input: SwapInput, onState: (state: SwapServiceState) => void): Promise<SwapService> {
  if (!/^\d+(\.\d+)?$/.test(input.amount) || !/[1-9]/.test(input.amount)) throw new Error("Invalid swap amount");
  if (input.provider === "near-intents") {
    const { createNearSwapService } = await import("./near-swap-service");
    return createNearSwapService(input, onState);
  }
  if (input.provider === "cow-swap") {
    const { createCowSwapService } = await import("./cow-swap-service");
    return createCowSwapService(input, onState);
  }
  if (input.provider === "symbiosis") {
    const { createBackendSwapService } = await import("./backend-swap-service");
    return createBackendSwapService(input, onState);
  }
  throw new Error("Wallet execution is unavailable for this provider");
}
