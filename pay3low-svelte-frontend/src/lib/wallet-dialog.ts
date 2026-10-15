import { get, writable } from "svelte/store";
import type { Session } from "$lib/otc/service";

export type WalletChain = "ethereum" | "everscale";
export type OtcWalletDialog = {
  session: Session | null;
  enabled: boolean;
  chains: WalletChain[];
  bound: string[];
  busy: boolean;
  disconnectDisabled: boolean;
  error: string;
  progress: string;
  requestingQuote: boolean;
  quoteReady: boolean;
  quoteStatus: string;
  onClose: () => void;
  onConnect: (chain: WalletChain) => Promise<void>;
  onVerify: (chain: WalletChain) => Promise<void>;
  onDisconnect: () => Promise<void>;
  onRequest: () => void;
};

export const otcWalletDialog = writable<OtcWalletDialog | null>(null);
export const walletDialogMode = writable<"wallets" | "otc" | null>(null);

export function openWalletDialog(mode: "wallets" | "otc") {
  walletDialogMode.set(mode);
}

export function closeWalletDialog() {
  const mode = get(walletDialogMode);
  walletDialogMode.set(null);
  if (mode === "otc") get(otcWalletDialog)?.onClose();
}
