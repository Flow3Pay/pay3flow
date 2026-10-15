import type { Page } from "@playwright/test";
import { readFileSync } from "node:fs";
import { encodeAbiParameters, encodeFunctionResult, type Address, type Hex } from "viem";
import { factoryAbi, safeAbi, SAFE_SINGLETON } from "../../src/lib/otc/production-safe";

// Official runtime bytecode captured read-only from Sepolia; hashes are checked in the unit suite.
export const safeFixture = JSON.parse(readFileSync(new URL("./safe-1.4.1.json", import.meta.url), "utf8")) as {
  contracts: Record<string, Hex>; creationCode: Hex;
};
export const owners = ["0x1111111111111111111111111111111111111111", "0x2222222222222222222222222222222222222222", "0x3333333333333333333333333333333333333333"] as Address[];
export const reference = `0x${"ab".repeat(32)}` as Hex;
export const storageKey = (chainId: number) => `pay3flow.otc.production-safe.v1.${chainId}`;

export async function mockSafeRpc(page: Page) {
  const state = { invalidContract: "", deployed: false, threshold: 2n, actualOwners: owners, chainId: null as number | null, calls: [] as string[] };
  await page.route(/^https:\/\/ethereum(?:-sepolia)?-rpc\.publicnode\.com\//, async route => {
    const request = route.request();
    const respond = ({ id, method, params }: { id: number; method: string; params: any[] }) => {
      state.calls.push(method);
      let result: unknown;
      if (method === "eth_chainId") result = `0x${(state.chainId ?? (request.url().includes("sepolia") ? 11155111 : 1)).toString(16)}`;
      else if (method === "eth_getCode") {
        const address = params[0].toLowerCase();
        result = address === state.invalidContract.toLowerCase() ? "0x6000" : safeFixture.contracts[address] ?? (state.deployed ? "0x6000" : "0x");
      } else if (method === "eth_call") {
        const data = params[0].data;
        if (data === "0x53e5d935") result = encodeFunctionResult({ abi: factoryAbi, functionName: "proxyCreationCode", result: safeFixture.creationCode });
        else if (data === "0xa0e67e2b") result = encodeFunctionResult({ abi: safeAbi, functionName: "getOwners", result: state.actualOwners });
        else if (data === "0xe75235b8") result = encodeFunctionResult({ abi: safeAbi, functionName: "getThreshold", result: state.threshold });
        else throw new Error(`Unexpected Safe call: ${data}`);
      } else if (method === "eth_getStorageAt") result = encodeAbiParameters([{ type: "address" }], [SAFE_SINGLETON]);
      else throw new Error(`Unexpected Safe RPC method: ${method}`);
      return { jsonrpc: "2.0", id, result };
    };
    const body = request.postDataJSON();
    await route.fulfill({ json: Array.isArray(body) ? body.map(respond) : respond(body) });
  });
  return state;
}

export async function mockSafeWallet(page: Page, options: { address?: string; outcome?: "submitted" | "unknown" | "rejected"; refuseSwitch?: boolean } = {}) {
  await page.addInitScript(({ address, outcome, refuseSwitch, reference }) => {
    const state = window as any;
    state.safeWallet = { address, chainId: "0x1", calls: [], sends: [], switches: [], outcome, refuseSwitch, hold: false, release: null };
    const listeners = new Map<string, Array<(value: unknown) => void>>();
    state.ethereum = {
      on: (event: string, callback: (value: unknown) => void) => listeners.set(event, [...(listeners.get(event) ?? []), callback]),
      removeListener: (event: string, callback: unknown) => listeners.set(event, (listeners.get(event) ?? []).filter(item => item !== callback)),
      request: async ({ method, params }: { method: string; params: any[] }) => {
        const wallet = state.safeWallet;
        wallet.calls.push(method);
        if (method === "eth_accounts" || method === "eth_requestAccounts") return [wallet.address];
        if (method === "eth_chainId") return wallet.chainId;
        if (method === "wallet_switchEthereumChain") {
          wallet.switches.push(params[0].chainId);
          if (wallet.refuseSwitch) throw Object.assign(new Error("Switch rejected"), { code: 4001 });
          wallet.chainId = params[0].chainId;
          for (const callback of listeners.get("chainChanged") ?? []) callback(wallet.chainId);
          return null;
        }
        if (method === "eth_sendTransaction") {
          wallet.sends.push(params[0]);
          if (wallet.hold) await new Promise(resolve => wallet.release = resolve);
          if (wallet.outcome === "unknown") throw new Error("Provider response lost");
          if (wallet.outcome === "rejected") throw Object.assign(new Error("User rejected creation"), { code: 4001 });
          return reference;
        }
        throw Object.assign(new Error(`Unexpected wallet method: ${method}`), { code: 4200 });
      },
    };
  }, { address: options.address ?? owners[0], outcome: options.outcome ?? "submitted", refuseSwitch: options.refuseSwitch ?? false, reference });
}
