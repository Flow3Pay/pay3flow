import { env } from "$env/dynamic/public";
import type { RouteExecution, RouteExecutionAction } from "$lib/exchange";
import type { AppKitNetwork } from "@reown/appkit/networks";
import type { EIP1193Provider } from "viem";

type Hex = `0x${string}`;
type Address = `0x${string}`;

export interface ConnectedWallet {
  family: "evm" | "near";
  address: string;
  chainId?: number;
}

interface EvmConnection extends ConnectedWallet {
  family: "evm";
  address: Address;
  chainId: number;
  config: unknown;
}

interface NearConnection extends ConnectedWallet {
  family: "near";
  selector: Awaited<ReturnType<typeof import("@near-wallet-selector/core")["setupWalletSelector"]>>;
}

let evmApp: Promise<{ modal: import("@reown/appkit").AppKit; config: unknown }> | null = null;
let nearApp: Promise<NearConnection["selector"]> | null = null;
let injectedEvmApp: Promise<{ config: unknown; connector: unknown }> | null = null;

function injectedWalletTarget() {
  const provider = window.ethereum as (EIP1193Provider & {
    off?: EIP1193Provider["removeListener"];
  }) | undefined;
  if (!provider) throw new Error("No injected EVM wallet was found in this browser");

  // Some wallet extensions expose an EIP-1193 proxy whose `removeListener`
  // getter returns a newly-bound function. That violates the Proxy invariant
  // for its non-configurable property and makes wagmi's injected connector
  // throw while inspecting the provider. Give wagmi a plain adapter instead;
  // keep request/event methods bound to the extension's original provider.
  return {
    id: "pay3flow-injected",
    name: "Browser wallet",
    provider: {
      request: provider.request.bind(provider),
      on: provider.on.bind(provider),
      removeListener: (event, listener) => {
        // Prefer the equivalent `off` API where available. Do not read the
        // broken `removeListener` property from the proxied extension object.
        try {
          provider.off?.call(provider, event, listener);
        } catch {
          // Event subscriptions are scoped to the cached connector lifetime.
        }
      },
    } satisfies EIP1193Provider,
  };
}

export interface PreparedWalletAction {
  expectedOutput?: string;
  expectedFee?: { asset: string; amount: string };
  expiresAt?: string;
  submit: () => Promise<
    | { reference: string; kind: "transaction_hash" | "order_uid" }
    | { kind: "approval_confirmed" }
  >;
}

const CHAIN_NAMES: Record<string, number> = {
  ethereum: 1,
  gnosis: 100,
  "arbitrum-one": 42161,
  base: 8453,
  "polygon-pos": 137,
  "avalanche-c": 43114,
  "bnb-smart-chain": 56,
  optimism: 10,
};

const erc20Abi = [
  { type: "function", name: "balanceOf", stateMutability: "view", inputs: [{ name: "account", type: "address" }], outputs: [{ name: "balance", type: "uint256" }] },
  { type: "function", name: "allowance", stateMutability: "view", inputs: [{ name: "owner", type: "address" }, { name: "spender", type: "address" }], outputs: [{ name: "amount", type: "uint256" }] },
  { type: "function", name: "approve", stateMutability: "nonpayable", inputs: [{ name: "spender", type: "address" }, { name: "amount", type: "uint256" }], outputs: [{ name: "ok", type: "bool" }] },
  { type: "function", name: "transfer", stateMutability: "nonpayable", inputs: [{ name: "recipient", type: "address" }, { name: "amount", type: "uint256" }], outputs: [{ name: "ok", type: "bool" }] },
] as const;

function networkOf(asset: string): string {
  return asset.split("@", 2)[1]?.toLowerCase() ?? "";
}

export function walletFamily(network: string): ConnectedWallet["family"] | null {
  if (network.toLowerCase() === "near") return "near";
  return CHAIN_NAMES[network.toLowerCase()] ? "evm" : null;
}

export function chainIdForNetwork(network: string): number | null {
  return CHAIN_NAMES[network.toLowerCase()] ?? null;
}

async function appKit() {
  if (evmApp) return evmApp;
  evmApp = (async () => {
    const projectId = env.PUBLIC_REOWN_PROJECT_ID?.trim();
    if (!projectId) throw new Error("PUBLIC_REOWN_PROJECT_ID is not configured");
    const [{ createAppKit }, { WagmiAdapter }, networks] = await Promise.all([
      import("@reown/appkit"),
      import("@reown/appkit-adapter-wagmi"),
      import("@reown/appkit/networks"),
    ]);
    const supported: [AppKitNetwork, ...AppKitNetwork[]] = [networks.mainnet, networks.gnosis, networks.arbitrum, networks.base, networks.polygon, networks.avalanche, networks.bsc, networks.optimism];
    const adapter = new WagmiAdapter({ projectId, networks: supported });
    const modal = createAppKit({
      adapters: [adapter],
      networks: supported,
      projectId,
      metadata: {
        name: "Pay3Flow",
        description: "Non-custodial route execution",
        url: window.location.origin,
        icons: [`${window.location.origin}/icons/assets/pay3flow_logo.svg`],
      },
      features: { analytics: false, email: false, socials: false },
    });
    return { modal, config: adapter.wagmiConfig };
  })();
  return evmApp;
}

async function waitForEvmAddress(modal: import("@reown/appkit").AppKit): Promise<Address> {
  const existing = modal.getAddress("eip155");
  if (existing?.startsWith("0x")) return existing as Address;
  return new Promise<Address>((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      unsubscribe();
      reject(new Error("Wallet connection timed out"));
    }, 120_000);
    const unsubscribe = modal.subscribeAccount((state) => {
      if (!state.address?.startsWith("0x")) return;
      window.clearTimeout(timeout);
      unsubscribe();
      resolve(state.address as Address);
    }, "eip155");
    void modal.open({ view: "Connect", namespace: "eip155" });
  });
}

export async function connectEvm(network: string): Promise<EvmConnection> {
  const chainId = chainIdForNetwork(network);
  if (!chainId) throw new Error(`Embedded EVM execution is unavailable for ${network}`);
  if (!env.PUBLIC_REOWN_PROJECT_ID?.trim()) {
    if (!injectedEvmApp) {
      injectedEvmApp = (async () => {
        const [{ createConfig, http }, { injected }, networks] = await Promise.all([
          import("wagmi"),
          import("wagmi/connectors"),
          import("@reown/appkit/networks"),
        ]);
        const supported = [networks.mainnet, networks.gnosis, networks.arbitrum, networks.base, networks.polygon, networks.avalanche, networks.bsc, networks.optimism] as const;
        const connector = injected({ target: injectedWalletTarget });
        const transports = Object.fromEntries(supported.map((chain) => [chain.id, http()])) as Record<(typeof supported)[number]["id"], ReturnType<typeof http>>;
        const config = createConfig({
          chains: supported,
          connectors: [connector],
          transports,
        });
        return { config, connector };
      })();
    }
    const { config, connector } = await injectedEvmApp;
    const { connect, switchChain } = await import("@wagmi/core");
    const connected = await connect(config as Parameters<typeof connect>[0], { connector: connector as Parameters<typeof connect>[1]["connector"] });
    if (!connected.accounts[0]) throw new Error("Injected wallet did not return an account");
    if (connected.chainId !== chainId) await switchChain(config as Parameters<typeof switchChain>[0], { chainId });
    return { family: "evm", address: connected.accounts[0] as Address, chainId, config };
  }
  const { modal, config } = await appKit();
  const address = await waitForEvmAddress(modal);
  const { switchChain } = await import("@wagmi/core");
  await switchChain(config as Parameters<typeof switchChain>[0], { chainId });
  return { family: "evm", address, chainId, config };
}

async function nearSelector() {
  if (!nearApp) {
    nearApp = (async () => {
      const [{ setupWalletSelector }, { setupMyNearWallet }] = await Promise.all([
        import("@near-wallet-selector/core"),
        import("@near-wallet-selector/my-near-wallet"),
      ]);
      return setupWalletSelector({ network: "mainnet", modules: [setupMyNearWallet()] });
    })();
  }
  return nearApp;
}

export async function connectNear(): Promise<NearConnection> {
  const selector = await nearSelector();
  let account = selector.store.getState().accounts.find((item) => item.active);
  if (!account) {
    const { setupModal } = await import("@near-wallet-selector/modal-ui");
    setupModal(selector, { contractId: "intents.near" }).show();
    account = await new Promise((resolve, reject) => {
      const timeout = window.setTimeout(() => {
        subscription.unsubscribe();
        reject(new Error("NEAR wallet connection timed out"));
      }, 120_000);
      const subscription = selector.store.observable.subscribe((state) => {
        const active = state.accounts.find((item) => item.active);
        if (!active) return;
        window.clearTimeout(timeout);
        subscription.unsubscribe();
        resolve(active);
      });
    });
  }
  if (!account) throw new Error("NEAR wallet did not return an active account");
  return { family: "near", address: account.accountId, selector };
}

export async function connectForNetwork(network: string): Promise<ConnectedWallet> {
  const family = walletFamily(network);
  if (family === "near") return connectNear();
  if (family === "evm") return connectEvm(network);
  throw new Error(`Wallet execution is not available for ${network}`);
}

function asEvm(wallet: ConnectedWallet): EvmConnection {
  if (wallet.family !== "evm") throw new Error("This action requires an EVM wallet");
  return wallet as EvmConnection;
}

function asNear(wallet: ConnectedWallet): NearConnection {
  if (wallet.family !== "near") throw new Error("This action requires a NEAR wallet");
  return wallet as NearConnection;
}

async function ensureEvmChain(wallet: EvmConnection): Promise<void> {
  const { getChainId, switchChain } = await import("@wagmi/core");
  const config = wallet.config as Parameters<typeof getChainId>[0];
  if (getChainId(config) !== wallet.chainId) {
    await switchChain(wallet.config as Parameters<typeof switchChain>[0], { chainId: wallet.chainId });
  }
}

function isNativeToken(token?: string): boolean {
  if (!token) return true;
  return /^0x0{40}$/i.test(token) || /^0x[e]{40}$/i.test(token);
}

export async function hasExecutionFunds(execution: RouteExecution, wallet: ConnectedWallet): Promise<boolean> {
  const action = execution.action;
  if (action.kind === "near_deposit" && action.network === "near") {
    const amount = decimalToAtomic(action.amount, action.decimals ?? 24);
    const balance = await nearBalance(asNear(wallet), action.token_contract);
    return balance >= amount;
  }
  const evm = asEvm(wallet);
  await ensureEvmChain(evm);
  const { getBalance, readContract } = await import("@wagmi/core");
  const required = requiredAtomic(action);
  const token = sourceToken(action);
  if (isNativeToken(token)) {
    const balance = await getBalance(evm.config as Parameters<typeof getBalance>[0], { address: evm.address, chainId: evm.chainId });
    return balance.value >= required;
  }
  const balance = await readContract(evm.config as Parameters<typeof readContract>[0], {
    address: token as Address,
    abi: erc20Abi,
    functionName: "balanceOf",
    args: [evm.address],
    chainId: evm.chainId,
  });
  return balance >= required;
}

export async function executeWalletAction(execution: RouteExecution, wallet: ConnectedWallet): Promise<
  | { reference: string; kind: "transaction_hash" | "order_uid" }
  | { kind: "approval_confirmed" }
> {
  return (await prepareWalletAction(execution, wallet)).submit();
}

export async function prepareWalletAction(execution: RouteExecution, wallet: ConnectedWallet): Promise<PreparedWalletAction> {
  const action = execution.action;
  if (wallet.family === "evm") await ensureEvmChain(asEvm(wallet));
  if (action.kind === "cow_order") {
    const evm = asEvm(wallet);
    if (evm.chainId !== action.chain_id) throw new Error("Connected wallet is on the wrong CoW network");
    const [{ getPublicClient, getWalletClient, waitForTransactionReceipt }, { TradingSdk, OrderKind }, { ViemAdapter }] = await Promise.all([
      import("@wagmi/core"),
      import("@cowprotocol/cow-sdk"),
      import("@cowprotocol/sdk-viem-adapter"),
    ]);
    const publicClient = getPublicClient(evm.config as Parameters<typeof getPublicClient>[0], { chainId: evm.chainId });
    const walletClient = await getWalletClient(evm.config as Parameters<typeof getWalletClient>[0], { chainId: evm.chainId });
    if (!publicClient || !walletClient) throw new Error("Connected wallet client is unavailable");
    const sdk = new TradingSdk({ chainId: action.chain_id, appCode: "Pay3Flow" }, {}, new ViemAdapter({ provider: publicClient, walletClient }));
    const trade = {
      kind: OrderKind.SELL,
      sellToken: action.sell_token,
      sellTokenDecimals: tokenDecimals(assetSymbol(execution.from_asset), 18),
      buyToken: action.buy_token,
      buyTokenDecimals: tokenDecimals(assetSymbol(execution.to_asset), 18),
      amount: action.sell_amount,
      receiver: execution.recipient,
    };
    const { quoteResults, postSwapOrderFromQuote } = await sdk.getQuote(trade);
    const atomicOutput = findBuyAmount(quoteResults);
    if (!atomicOutput) throw new Error("CoW quote did not include the expected output amount");
    const expectedOutput = atomicToDecimal(atomicOutput, trade.buyTokenDecimals);
    const atomicFee = findFeeAmount(quoteResults);
    return {
      expectedOutput,
      expectedFee: atomicFee
        ? { asset: execution.from_asset, amount: atomicToDecimal(atomicFee, trade.sellTokenDecimals) }
        : undefined,
      expiresAt: findQuoteExpiry(quoteResults),
      submit: async () => {
        const amount = BigInt(action.sell_amount);
        const allowance = await sdk.getCowProtocolAllowance({ tokenAddress: action.sell_token, owner: evm.address, chainId: action.chain_id });
        if (allowance < amount) {
          const approvalHash = await sdk.approveCowProtocol({ tokenAddress: action.sell_token, amount, chainId: action.chain_id });
          await waitForTransactionReceipt(evm.config as Parameters<typeof waitForTransactionReceipt>[0], { hash: approvalHash as Hex, chainId: evm.chainId });
          return { kind: "approval_confirmed" };
        }
        const result = await postSwapOrderFromQuote();
        return { reference: result.orderId, kind: "order_uid" };
      },
    };
  }
  return { submit: () => submitNonCowAction(execution, wallet) };
}

async function submitNonCowAction(execution: RouteExecution, wallet: ConnectedWallet): Promise<
  | { reference: string; kind: "transaction_hash" | "order_uid" }
  | { kind: "approval_confirmed" }
> {
  const action = execution.action;
  if (action.kind === "near_deposit") {
    if (action.network === "near") return executeNearDeposit(action, asNear(wallet));
    return executeEvmDeposit(action, asEvm(wallet));
  }
  if (action.kind === "symbiosis_transaction") return executeSymbiosis(action, asEvm(wallet));
  throw new Error("Unsupported wallet action");
}

async function executeEvmDeposit(action: Extract<RouteExecutionAction, { kind: "near_deposit" }>, wallet: EvmConnection) {
  const { sendTransaction, writeContract } = await import("@wagmi/core");
  const amount = decimalToAtomic(action.amount, action.decimals ?? 18);
  const reference = action.token_contract && !isNativeToken(action.token_contract)
    ? await writeContract(wallet.config as Parameters<typeof writeContract>[0], {
        address: action.token_contract as Address,
        abi: erc20Abi,
        functionName: "transfer",
        args: [action.deposit_address as Address, amount],
        chainId: wallet.chainId,
      })
    : await sendTransaction(wallet.config as Parameters<typeof sendTransaction>[0], {
        to: action.deposit_address as Address,
        value: amount,
        chainId: wallet.chainId,
      });
  return { reference, kind: "transaction_hash" as const };
}

async function executeNearDeposit(action: Extract<RouteExecutionAction, { kind: "near_deposit" }>, wallet: NearConnection) {
  const selected = await wallet.selector.wallet();
  const { actionCreators } = await import("@near-js/transactions");
  const amount = decimalToAtomic(action.amount, action.decimals ?? 24).toString();
  const outcome = action.token_contract
    ? await selected.signAndSendTransaction({
        receiverId: action.token_contract,
        actions: [actionCreators.functionCall("ft_transfer", { receiver_id: action.deposit_address, amount, memo: action.deposit_memo }, 30_000_000_000_000n, 1n)],
      })
    : await selected.signAndSendTransaction({
        receiverId: action.deposit_address,
        actions: [actionCreators.transfer(BigInt(amount))],
      });
  const reference = outcome?.transaction?.hash ?? outcome?.transaction_outcome?.id;
  if (!reference) throw new Error("NEAR wallet did not return a transaction hash");
  return { reference, kind: "transaction_hash" as const };
}

async function executeCow(action: Extract<RouteExecutionAction, { kind: "cow_order" }>, execution: RouteExecution, wallet: EvmConnection) {
  const [{ getPublicClient, getWalletClient, waitForTransactionReceipt }, { TradingSdk, OrderKind }, { ViemAdapter }] = await Promise.all([
    import("@wagmi/core"),
    import("@cowprotocol/cow-sdk"),
    import("@cowprotocol/sdk-viem-adapter"),
  ]);
  if (wallet.chainId !== action.chain_id) throw new Error("Connected wallet is on the wrong CoW network");
  const publicClient = getPublicClient(wallet.config as Parameters<typeof getPublicClient>[0], { chainId: wallet.chainId });
  const walletClient = await getWalletClient(wallet.config as Parameters<typeof getWalletClient>[0], { chainId: wallet.chainId });
  if (!publicClient || !walletClient) throw new Error("Connected wallet client is unavailable");
  const sdk = new TradingSdk({ chainId: action.chain_id, appCode: "Pay3Flow" }, {}, new ViemAdapter({ provider: publicClient, walletClient }));
  const amount = BigInt(action.sell_amount);
  const allowance = await sdk.getCowProtocolAllowance({ tokenAddress: action.sell_token, owner: wallet.address, chainId: action.chain_id });
  if (allowance < amount) {
    const approvalHash = await sdk.approveCowProtocol({ tokenAddress: action.sell_token, amount, chainId: action.chain_id });
    await waitForTransactionReceipt(wallet.config as Parameters<typeof waitForTransactionReceipt>[0], { hash: approvalHash as Hex, chainId: wallet.chainId });
  }
  const trade = {
    kind: OrderKind.SELL,
    sellToken: action.sell_token,
    sellTokenDecimals: tokenDecimals(assetSymbol(execution.from_asset), 18),
    buyToken: action.buy_token,
    buyTokenDecimals: tokenDecimals(assetSymbol(execution.to_asset), 18),
    amount: action.sell_amount,
    receiver: execution.recipient,
  };
  const { postSwapOrderFromQuote } = await sdk.getQuote(trade);
  const result = await postSwapOrderFromQuote();
  return { reference: result.orderId, kind: "order_uid" as const };
}

async function executeSymbiosis(action: Extract<RouteExecutionAction, { kind: "symbiosis_transaction" }>, wallet: EvmConnection) {
  const { readContract, sendTransaction, waitForTransactionReceipt, writeContract } = await import("@wagmi/core");
  if (wallet.chainId !== action.chain_id) throw new Error("Connected wallet is on the wrong Symbiosis network");
  const required = BigInt(action.input_amount);
  if (action.approval_spender && !isNativeToken(action.source_token)) {
    const allowance = await readContract(wallet.config as Parameters<typeof readContract>[0], {
      address: action.source_token as Address,
      abi: erc20Abi,
      functionName: "allowance",
      args: [wallet.address, action.approval_spender as Address],
      chainId: wallet.chainId,
    });
    if (allowance < required) {
      const approvalHash = await writeContract(wallet.config as Parameters<typeof writeContract>[0], {
        address: action.source_token as Address,
        abi: erc20Abi,
        functionName: "approve",
        args: [action.approval_spender as Address, required],
        chainId: wallet.chainId,
      });
      await waitForTransactionReceipt(wallet.config as Parameters<typeof waitForTransactionReceipt>[0], { hash: approvalHash, chainId: wallet.chainId });
      return { kind: "approval_confirmed" as const };
    }
  }
  const tx = action.transaction;
  const reference = await sendTransaction(wallet.config as Parameters<typeof sendTransaction>[0], {
    chainId: wallet.chainId,
    to: String(tx.to) as Address,
    data: String(tx.data) as Hex,
    value: tx.value == null ? 0n : BigInt(String(tx.value)),
  });
  return { reference, kind: "transaction_hash" as const };
}

function sourceToken(action: RouteExecutionAction): string | undefined {
  if (action.kind === "cow_order") return action.sell_token;
  if (action.kind === "symbiosis_transaction") return action.source_token;
  return action.token_contract;
}

function requiredAtomic(action: RouteExecutionAction): bigint {
  if (action.kind === "cow_order") return BigInt(action.sell_amount);
  if (action.kind === "symbiosis_transaction") return BigInt(action.input_amount);
  return decimalToAtomic(action.amount, action.decimals ?? 18);
}

function decimalToAtomic(value: string, decimals: number): bigint {
  const [whole = "0", fraction = ""] = value.trim().split(".", 2);
  const normalized = `${whole || "0"}${fraction.padEnd(decimals, "0").slice(0, decimals)}`.replace(/^0+(?=\d)/, "");
  return BigInt(normalized || "0");
}

async function nearBalance(wallet: NearConnection, tokenContract?: string): Promise<bigint> {
  const query = tokenContract
    ? { request_type: "call_function", finality: "final", account_id: tokenContract, method_name: "ft_balance_of", args_base64: btoa(JSON.stringify({ account_id: wallet.address })) }
    : { request_type: "view_account", finality: "final", account_id: wallet.address };
  const response = await fetch("https://rpc.mainnet.near.org", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ jsonrpc: "2.0", id: "pay3flow-balance", method: "query", params: query }) });
  if (!response.ok) throw new Error("Unable to read NEAR balance");
  const payload = await response.json() as { result?: { amount?: string; result?: number[] } };
  if (!tokenContract) return BigInt(payload.result?.amount ?? "0");
  const bytes = payload.result?.result ?? [];
  return BigInt(JSON.parse(new TextDecoder().decode(Uint8Array.from(bytes))) as string);
}

function assetSymbol(asset: string): string {
  return asset.split("@", 1)[0] ?? asset;
}

function tokenDecimals(symbol: string, fallback: number): number {
  return ["USDC", "USDT", "FDUSD"].includes(symbol.toUpperCase()) ? 6 : fallback;
}

function findBuyAmount(value: unknown): string | undefined {
  if (Array.isArray(value)) return value.map(findBuyAmount).find((amount) => amount !== undefined);
  if (typeof value !== "object" || value === null) return undefined;
  const record = value as Record<string, unknown>;
  const amount = record.buyAmount ?? record.buy_amount;
  if (typeof amount === "string" && /^\d+$/.test(amount)) return amount;
  return Object.values(record).map(findBuyAmount).find((candidate) => candidate !== undefined);
}

function findFeeAmount(value: unknown): string | undefined {
  if (Array.isArray(value)) return value.map(findFeeAmount).find((amount) => amount !== undefined);
  if (typeof value !== "object" || value === null) return undefined;
  const record = value as Record<string, unknown>;
  const amount = record.feeAmount ?? record.fee_amount;
  if (typeof amount === "string" && /^\d+$/.test(amount)) return amount;
  return Object.values(record).map(findFeeAmount).find((candidate) => candidate !== undefined);
}

function findQuoteExpiry(value: unknown): string | undefined {
  if (Array.isArray(value)) return value.map(findQuoteExpiry).find((expiry) => expiry !== undefined);
  if (typeof value !== "object" || value === null) return undefined;
  const record = value as Record<string, unknown>;
  const validTo = record.validTo ?? record.valid_to;
  if ((typeof validTo === "number" || (typeof validTo === "string" && /^\d+$/.test(validTo))) && Number(validTo) > 0) {
    return new Date(Number(validTo) * 1000).toISOString();
  }
  return Object.values(record).map(findQuoteExpiry).find((expiry) => expiry !== undefined);
}

function atomicToDecimal(value: string, decimals: number): string {
  const padded = value.padStart(decimals + 1, "0");
  const split = padded.length - decimals;
  const fraction = padded.slice(split).replace(/0+$/, "");
  return fraction ? `${padded.slice(0, split)}.${fraction}` : padded.slice(0, split);
}

export function sourceNetwork(execution: { from_asset: string }): string {
  return networkOf(execution.from_asset);
}

export function targetNetwork(execution: { to_asset: string }): string {
  return networkOf(execution.to_asset);
}
