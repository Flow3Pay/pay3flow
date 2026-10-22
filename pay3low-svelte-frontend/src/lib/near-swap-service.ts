import type { SwapInput, SwapService, SwapServiceState } from "./swap-service";

/**
 * The NEAR Intents widget (and its React/Solana/Stellar dependency tree) has
 * been removed from this build to keep the frontend installable and fast to
 * deploy. Keep the module so the swap route still loads and reports a clear
 * temporary error instead of failing at import time.
 *
 * Re-enable the widget by restoring near-sdk.ts and the original service
 * implementation once the dependency stack is fixed.
 */
export function createNearSwapService(_input: SwapInput, _onState: (state: SwapServiceState) => void): SwapService {
  throw new Error("Swaps via NEAR Intents are temporarily unavailable. The exchange SDK is being replaced.");
}
