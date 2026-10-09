import { mount, unmount } from "svelte";
import RouteExecutionPanel from "../../src/lib/components/RouteExecutionPanel.svelte";
import RouteInstructions from "../../src/lib/components/RouteInstructions.svelte";
import { wallets } from "../../src/lib/wallet-session";
import { hasExecutionFunds, prepareWalletAction, type ConnectedWallet } from "../../src/lib/wallet-execution";
import type { RouteCandidate, RouteExecution } from "../../src/lib/exchange";
import { createConfig, connect, custom } from "@wagmi/core";
import { injected } from "wagmi/connectors";
import { avalanche, mainnet } from "viem/chains";

let panel: ReturnType<typeof mount> | null = null;
let nearAccount = "alice.near";
let nearListeners: Array<(state: unknown) => void> = [];

export async function nearWallet(): Promise<ConnectedWallet> {
  const selector = {
    store: { getState: () => ({ accounts: [{ active: true, accountId: nearAccount }] }), observable: {
      subscribe: (callback: (state: unknown) => void) => { nearListeners.push(callback); return { unsubscribe: () => { nearListeners = nearListeners.filter(item => item !== callback); } }; },
    } },
    wallet: async () => ({ signAndSendTransaction: async (input: unknown) => {
      (window as any).walletCalls.push(input);
      if ((window as any).rejectWallet) throw new Error("User cancelled signing");
      return { transaction: { hash: "7".repeat(44) } };
    } }),
  };
  return { family: "near", address: nearAccount, selector } as unknown as ConnectedWallet;
}

export async function evmWallet(): Promise<ConnectedWallet> {
  const provider = (window as any).ethereum;
  const config = createConfig({ batch: { multicall: false }, chains: [mainnet, avalanche], connectors: [injected({ target: { id: "test", name: "Test wallet", provider } })], transports: { [mainnet.id]: custom(provider), [avalanche.id]: custom(provider) } });
  const connection = await connect(config, { connector: config.connectors[0] });
  return { family: "evm", address: connection.accounts[0], chainId: connection.chainId, config };
}

export async function setEvmConnected(connected: boolean) {
  const wallet = connected ? await evmWallet() : null;
  wallets.update(state => {
    const next = { ...state };
    if (wallet) next.evm = wallet as Extract<ConnectedWallet, { family: "evm" }>;
    else delete next.evm;
    return next;
  });
}

export async function mountGuide(route: RouteCandidate) {
  if (panel) await unmount(panel);
  const target = document.createElement("div");
  target.id = "wallet-test-panel";
  document.body.appendChild(target);
  panel = mount(RouteInstructions, { target, props: { route, venueNames: { symbiosis: "Symbiosis" }, networkNames: { "avalanche-c": "Avalanche C-Chain" }, onClose: () => {} } });
}

export async function mountPanel(route: RouteCandidate, family: "near" | "evm" | "tron") {
  if (panel) await unmount(panel);
  const wallet = family === "near" ? await nearWallet() : family === "evm" ? await evmWallet() : null;
  if (wallet) wallets.update(state => ({ ...state, [family]: wallet }));
  const target = document.createElement("div");
  target.id = "wallet-test-panel";
  document.body.appendChild(target);
  panel = mount(RouteExecutionPanel, { target, props: { route } });
}

export async function perform(execution: RouteExecution, family: "near" | "evm") {
  const wallet = family === "near" ? await nearWallet() : await evmWallet();
  if (!await hasExecutionFunds(execution, wallet)) throw new Error("Insufficient balance");
  return (await prepareWalletAction(execution, wallet)).submit();
}

export function changeNearAccount(account: string) {
  nearAccount = account;
  for (const listener of nearListeners) listener({ accounts: [{ active: true, accountId: account }] });
}
