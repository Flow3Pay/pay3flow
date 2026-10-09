import { get, writable } from "svelte/store";
import { sameWalletAddress } from "./wallet-amount";
import { connectForNetwork, disconnectWallet, observeWallet, restoreWallet, walletFamily, type ConnectedWallet, type WalletFamily } from "./wallet-execution";

export const wallets = writable<Partial<Record<WalletFamily, ConnectedWallet>>>({});
const subscriptions = new Map<WalletFamily, () => void>();
const pending = new Map<WalletFamily, Promise<ConnectedWallet>>();
const generations = new Map<WalletFamily, number>();
const STORAGE_KEY = "pay3flow.wallet-families";
let restoration: Promise<void> | null = null;

function remember() {
  try { localStorage.setItem(STORAGE_KEY, JSON.stringify(Object.keys(get(wallets)))); } catch { /* Optional persistence. */ }
}

async function retain(wallet: ConnectedWallet, generation: number) {
  const family = wallet.family;
  subscriptions.get(family)?.();
  wallets.update(state => ({ ...state, [family]: wallet }));
  remember();
  const unsubscribe = await observeWallet(wallet, value => {
    if (generations.get(family) !== generation) return;
    const current = get(wallets)[family];
    if (current && value && sameWalletAddress(family, current.address, value.address)
      && (current.family !== "evm" || (value.family === "evm" && current.chainId === value.chainId))
      && (current.family !== "everscale" || (value.family === "everscale" && current.networkId === value.networkId))) return;
    wallets.update(state => {
      const next = { ...state };
      if (value) next[family] = value;
      else delete next[family];
      return next;
    });
    remember();
  });
  if (generations.get(family) !== generation) unsubscribe();
  else subscriptions.set(family, unsubscribe);
}

export async function connectWallet(network: string): Promise<ConnectedWallet> {
  const family = walletFamily(network);
  if (!family) throw new Error(`Wallet connection is unavailable for ${network}`);
  const inProgress = pending.get(family);
  if (inProgress) return inProgress;
  const generation = (generations.get(family) ?? 0) + 1;
  generations.set(family, generation);
  const promise = (async () => {
    const wallet = await connectForNetwork(network);
    if (generations.get(family) !== generation) throw new Error("Wallet connection was cancelled");
    await retain(wallet, generation);
    return wallet;
  })();
  pending.set(family, promise);
  try { return await promise; } finally { pending.delete(family); }
}

export async function disconnectFamily(family: WalletFamily) {
  generations.set(family, (generations.get(family) ?? 0) + 1);
  subscriptions.get(family)?.();
  subscriptions.delete(family);
  const wallet = get(wallets)[family];
  wallets.update(state => { const next = { ...state }; delete next[family]; return next; });
  remember();
  if (wallet) await disconnectWallet(wallet);
}

export function restoreWallets(): Promise<void> {
  if (restoration) return restoration;
  restoration = (async () => {
    let families: unknown;
    try { families = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "[]"); } catch { return; }
    if (!Array.isArray(families)) return;
    await Promise.all(families.filter((family): family is WalletFamily => ["evm", "near", "tron", "everscale"].includes(family)).map(async family => {
      const generation = generations.get(family) ?? 0;
      generations.set(family, generation);
      try {
        const wallet = await restoreWallet(family);
        if (wallet && generations.get(family) === generation) await retain(wallet, generation);
      } catch { /* Missing or locked wallets can be connected explicitly later. */ }
    }));
  })();
  return restoration;
}
