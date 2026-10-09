import { env } from "$env/dynamic/public";
import type { RouteExecution, RouteExecutionAction } from "$lib/exchange";
import type { AppKitNetwork } from "@reown/appkit/networks";
import type { EIP1193Provider } from "viem";
import { decimalToAtomic, atomicToDecimal, sameWalletAddress } from "./wallet-amount";
import { TRON_CHAIN_ID, connectTronProvider, ensureTronMainnet, tronProvider, tronAddress, broadcastTron, type TronProvider } from "./tron-wallet";

type Hex = `0x${string}`;
type Address = `0x${string}`;

export type ConnectedWallet = EvmConnection | NearConnection | TronConnection;
export type WalletFamily = ConnectedWallet["family"];

export interface EvmConnection {
  family: "evm";
  address: Address;
  chainId: number;
  config: unknown;
}

export interface NearConnection {
  family: "near";
  address: string;
  selector: Awaited<ReturnType<typeof import("@near-wallet-selector/core")["setupWalletSelector"]>>;
}

export interface TronConnection {
  family: "tron";
  address: string;
  chainId: number;
  provider: TronProvider;
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
  { type: "function", name: "decimals", stateMutability: "view", inputs: [], outputs: [{ name: "decimals", type: "uint8" }] },
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
  if (network.toLowerCase() === "tron") return "tron";
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
    let unsubscribeAccount = () => {};
    let unsubscribeState = () => {};
    let finished = false;
    const cleanup = () => { finished = true; window.clearTimeout(timeout); unsubscribeAccount(); unsubscribeState(); };
    const timeout = window.setTimeout(() => { cleanup(); reject(new Error("Wallet connection timed out")); }, 120_000);
    unsubscribeAccount = modal.subscribeAccount(state => {
      if (!state.address?.startsWith("0x")) return;
      cleanup(); resolve(state.address as Address);
    }, "eip155");
    let opened = false;
    unsubscribeState = modal.subscribeState(state => {
      if (state.open) opened = true;
      else if (opened && !finished && !modal.getAddress("eip155")) { cleanup(); reject(new Error("Wallet connection was cancelled")); }
    });
    if (finished) { unsubscribeAccount(); unsubscribeState(); return; }
    void modal.open({ view: "Connect", namespace: "eip155" }).catch(cause => { cleanup(); reject(cause); });
  });
}

async function injectedApp() {
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
  return injectedEvmApp!;
}

export async function connectEvm(network: string): Promise<EvmConnection> {
  const chainId = chainIdForNetwork(network);
  if (!chainId) throw new Error(`Embedded EVM execution is unavailable for ${network}`);
  if (!env.PUBLIC_REOWN_PROJECT_ID?.trim()) {
    const { config, connector } = await injectedApp();
    const { connect, switchChain, getAccount } = await import("@wagmi/core");
    const account = getAccount(config as Parameters<typeof getAccount>[0]);
    const connected = account.isConnected ? { accounts: account.addresses ?? [], chainId: account.chainId } : await connect(config as Parameters<typeof connect>[0], { connector: connector as Parameters<typeof connect>[1]["connector"] });
    if (!connected.accounts[0]) throw new Error("Injected wallet did not return an account");
    if (connected.chainId !== chainId) await switchChain(config as Parameters<typeof switchChain>[0], { chainId });
    const active = getAccount(config as Parameters<typeof getAccount>[0]);
    if (!active.address || active.chainId !== chainId) throw new Error("Wallet did not switch to the selected network");
    return { family: "evm", address: active.address, chainId, config };
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
    const modal = setupModal(selector, { contractId: "intents.near" });
    account = await new Promise((resolve, reject) => {
      let unsubscribe = () => {};
      let unsubscribeHide = () => {};
      let finished = false;
      const cleanup = () => { finished = true; window.clearTimeout(timeout); unsubscribe(); unsubscribeHide(); };
      const timeout = window.setTimeout(() => { cleanup(); reject(new Error("NEAR wallet connection timed out")); }, 120_000);
      const hide = modal.on("onHide", event => {
        if (event.hideReason === "user-triggered") { cleanup(); reject(new Error("NEAR wallet connection was cancelled")); }
      });
      unsubscribeHide = () => hide.remove();
      const subscription = selector.store.observable.subscribe(state => {
        const active = state.accounts.find(item => item.active);
        if (!active) return;
        cleanup(); resolve(active);
      });
      unsubscribe = () => subscription.unsubscribe();
      if (finished) { unsubscribe(); unsubscribeHide(); }
      else modal.show();
    });
  }
  if (!account) throw new Error("NEAR wallet did not return an active account");
  return { family: "near", address: account.accountId, selector };
}

export async function connectForNetwork(network: string): Promise<ConnectedWallet> {
  const family = walletFamily(network);
  if (family === "tron") {
    const provider = await connectTronProvider();
    const web = await ensureTronMainnet(provider);
    return { family: "tron", address: web.defaultAddress.base58, chainId: TRON_CHAIN_ID, provider };
  }
  if (family === "near") return connectNear();
  if (family === "evm") return connectEvm(network);
  throw new Error(`Wallet execution is not available for ${network}`);
}

function asEvm(wallet: ConnectedWallet): EvmConnection {
  if (wallet.family !== "evm") throw new Error("This action requires an EVM wallet");
  return wallet;
}

function asNear(wallet: ConnectedWallet): NearConnection {
  if (wallet.family !== "near") throw new Error("This action requires a NEAR wallet");
  return wallet;
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
  await assertExecutionWallet(execution, wallet);
  const action = execution.action;
  if (wallet.family === "tron") {
    const web = await ensureTronMainnet(wallet.provider);
    const token = sourceToken(action);
    const balance = isNativeToken(token)
      ? await web.trx.getBalance(wallet.address)
      : await (await web.contract(erc20Abi, tronAddress(web, token!))).balanceOf(wallet.address).call();
    return BigInt(balance.toString()) >= requiredAtomic(action);
  }
  if (action.kind === "near_deposit" && action.network === "near") {
    const amount = requiredAtomic(action);
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
  await assertExecutionWallet(execution, wallet);
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
    const sellDecimals = action.sell_token_decimals ?? await publicClient.readContract({ address: action.sell_token as Address, abi: erc20Abi, functionName: "decimals" });
    const buyDecimals = action.buy_token_decimals ?? await publicClient.readContract({ address: action.buy_token as Address, abi: erc20Abi, functionName: "decimals" });
    const trade = {
      kind: OrderKind.SELL,
      sellToken: action.sell_token,
      sellTokenDecimals: sellDecimals,
      buyToken: action.buy_token,
      buyTokenDecimals: buyDecimals,
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
        await assertReadyToSign(execution, wallet);
        const amount = BigInt(action.sell_amount);
        const allowance = await sdk.getCowProtocolAllowance({ tokenAddress: action.sell_token, owner: evm.address, chainId: action.chain_id });
        if (allowance < amount) {
          const approvalHash = await sdk.approveCowProtocol({ tokenAddress: action.sell_token, amount, chainId: action.chain_id });
          const receipt = await waitForTransactionReceipt(evm.config as Parameters<typeof waitForTransactionReceipt>[0], { hash: approvalHash as Hex, chainId: evm.chainId });
          if (receipt.status !== "success") throw new Error("Token approval failed");
          return { kind: "approval_confirmed" };
        }
        await assertReadyToSign(execution, wallet);
        const result = await postSwapOrderFromQuote();
        return { reference: result.orderId, kind: "order_uid" };
      },
    };
  }
  return { submit: async () => {
    await assertReadyToSign(execution, wallet);
    return submitNonCowAction(execution, wallet);
  } };
}

async function submitNonCowAction(execution: RouteExecution, wallet: ConnectedWallet): Promise<
  | { reference: string; kind: "transaction_hash" | "order_uid" }
  | { kind: "approval_confirmed" }
> {
  const action = execution.action;
  if (action.kind === "near_deposit") {
    if (wallet.family === "tron") return executeTronDeposit(action, wallet);
    if (action.network === "near") return executeNearDeposit(action, asNear(wallet), execution.id);
    return executeEvmDeposit(action, asEvm(wallet));
  }
  if (action.kind === "symbiosis_transaction") {
    if (wallet.family === "tron") return executeTronSymbiosis(action, wallet);
    return executeSymbiosis(action, asEvm(wallet));
  }
  throw new Error("Unsupported wallet action");
}

async function executeEvmDeposit(action: Extract<RouteExecutionAction, { kind: "near_deposit" }>, wallet: EvmConnection) {
  const { sendTransaction, writeContract } = await import("@wagmi/core");
  const amount = requiredAtomic(action);
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

async function executeNearDeposit(action: Extract<RouteExecutionAction, { kind: "near_deposit" }>, wallet: NearConnection, executionId: string) {
  const selected = await wallet.selector.wallet();
  const { actionCreators } = await import("@near-js/transactions");
  const amount = requiredAtomic(action).toString();
  const callback = new URL(window.location.href);
  callback.searchParams.set("pay3flow_execution", executionId);
  const outcome = action.token_contract
    ? await selected.signAndSendTransaction({
        receiverId: action.token_contract,
        callbackUrl: callback.toString(),
        actions: [actionCreators.functionCall("ft_transfer", new TextEncoder().encode(JSON.stringify({ receiver_id: action.deposit_address, amount, memo: action.deposit_memo })), 30_000_000_000_000n, 1n)],
      })
    : await selected.signAndSendTransaction({
        receiverId: action.deposit_address,
        callbackUrl: callback.toString(),
        actions: [actionCreators.transfer(BigInt(amount))],
      });
  const reference = outcome?.transaction?.hash ?? outcome?.transaction_outcome?.id;
  if (!reference) throw new Error("NEAR wallet did not return a transaction hash");
  return { reference, kind: "transaction_hash" as const };
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
      const receipt = await waitForTransactionReceipt(wallet.config as Parameters<typeof waitForTransactionReceipt>[0], { hash: approvalHash, chainId: wallet.chainId });
      if (receipt.status !== "success") throw new Error("Token approval failed");
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
  if (action.decimals == null) throw new Error("Provider did not supply token precision");
  return decimalToAtomic(action.amount, action.decimals);
}

async function nearBalance(wallet: NearConnection, tokenContract?: string): Promise<bigint> {
  const query = tokenContract
    ? { request_type: "call_function", finality: "final", account_id: tokenContract, method_name: "ft_balance_of", args_base64: btoa(JSON.stringify({ account_id: wallet.address })) }
    : { request_type: "view_account", finality: "final", account_id: wallet.address };
  const response = await fetch("https://rpc.mainnet.near.org", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ jsonrpc: "2.0", id: "pay3flow-balance", method: "query", params: query }) });
  if (!response.ok) throw new Error("Unable to read NEAR balance");
  const payload = await response.json() as { error?: unknown; result?: { amount?: string; result?: number[] } };
  if (payload.error || !payload.result) throw new Error("Unable to read NEAR balance");
  if (!tokenContract) return BigInt(payload.result?.amount ?? "0");
  const bytes = payload.result?.result ?? [];
  return BigInt(JSON.parse(new TextDecoder().decode(Uint8Array.from(bytes))) as string);
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

export function sourceNetwork(execution: { from_asset: string }): string {
  return networkOf(execution.from_asset);
}

export function targetNetwork(execution: { to_asset: string }): string {
  return networkOf(execution.to_asset);
}

export async function validateRecipient(network: string, address: string): Promise<void> {
  if (walletFamily(network) === "evm") {
    const { isAddress } = await import("viem");
    if (!isAddress(address) || /^0x0{40}$/i.test(address)) throw new Error("Invalid EVM recipient address");
  } else if (network === "near") {
    if (!/^(?=.{2,64}$)[a-z0-9]+(?:[._-][a-z0-9]+)*$/.test(address)) throw new Error("Invalid NEAR recipient account");
  } else if (network === "tron") {
    const { isTronAddress } = await import("./wallet-address");
    if (!await isTronAddress(address)) throw new Error("Invalid TRON recipient address");
  } else {
    throw new Error(`Recipient validation is unavailable for ${network}`);
  }
}

async function assertExecutionWallet(execution: RouteExecution, wallet: ConnectedWallet) {
  const network = sourceNetwork(execution);
  if (walletFamily(network) !== wallet.family || !sameWalletAddress(wallet.family, wallet.address, execution.source_address)) throw new Error("Reconnect the source wallet used for this swap");
  if (wallet.family === "evm") {
    const { getAccount } = await import("@wagmi/core");
    const account = getAccount(wallet.config as Parameters<typeof getAccount>[0]);
    if (!account.address || !sameWalletAddress("evm", account.address, wallet.address) || account.chainId !== chainIdForNetwork(network)) throw new Error("Source wallet account or network changed. Reconnect to continue.");
  } else if (wallet.family === "near") {
    const account = wallet.selector.store.getState().accounts.find(item => item.active);
    if (account?.accountId !== wallet.address) throw new Error("NEAR wallet account changed or disconnected");
  } else {
    const web = await ensureTronMainnet(wallet.provider);
    if (web.defaultAddress.base58 !== wallet.address) throw new Error("TRON wallet account changed");
  }
}

async function assertReadyToSign(execution: RouteExecution, wallet: ConnectedWallet) {
  if (execution.status !== "awaiting_signature") throw new Error("This swap has already been submitted");
  if (!Number.isFinite(Date.parse(execution.quote_expires_at)) || Date.parse(execution.quote_expires_at) <= Date.now()) throw new Error("Swap quote expired. Prepare a fresh quote.");
  if (!await hasExecutionFunds(execution, wallet)) throw new Error("Insufficient source token balance");
}

async function executeTronDeposit(action: Extract<RouteExecutionAction, { kind: "near_deposit" }>, wallet: TronConnection) {
  const web = await ensureTronMainnet(wallet.provider);
  const amount = requiredAtomic(action).toString();
  const recipient = tronAddress(web, action.deposit_address);
  if (action.deposit_memo) throw new Error("TRON deposit memos are not supported");
  let reference: string;
  if (isNativeToken(action.token_contract)) {
    // TronWeb converts SUN to a JavaScript number internally.
    if (BigInt(amount) > BigInt(Number.MAX_SAFE_INTEGER)) throw new Error("TRX amount exceeds the wallet's exact transfer limit");
    reference = await broadcastTron(web, await web.transactionBuilder.sendTrx(recipient, amount, wallet.address));
  } else {
    const contract = await web.contract(erc20Abi, tronAddress(web, action.token_contract!));
    const result = await contract.transfer(recipient, amount).send({ feeLimit: 100_000_000 });
    reference = Array.isArray(result) ? result[0] : result;
  }
  if (!/^[a-f\d]{64}$/i.test(reference)) throw new Error("TRON wallet did not return a transaction hash");
  return { reference, kind: "transaction_hash" as const };
}

async function executeTronSymbiosis(action: Extract<RouteExecutionAction, { kind: "symbiosis_transaction" }>, wallet: TronConnection) {
  if (action.chain_id !== TRON_CHAIN_ID) throw new Error("Symbiosis transaction is on the wrong TRON network");
  const web = await ensureTronMainnet(wallet.provider);
  if (!isNativeToken(action.source_token)) {
    if (!action.approval_spender) throw new Error("Symbiosis did not supply an approval spender");
    const contract = await web.contract(erc20Abi, tronAddress(web, action.source_token));
    const spender = tronAddress(web, action.approval_spender);
    const allowance = await contract.allowance(wallet.address, spender).call();
    if (BigInt(allowance.toString()) < BigInt(action.input_amount)) {
      await contract.approve(spender, action.input_amount).send({ feeLimit: 100_000_000, shouldPollResponse: true, keepTxID: true });
      const confirmed = await contract.allowance(wallet.address, spender).call();
      if (BigInt(confirmed.toString()) < BigInt(action.input_amount)) throw new Error("TRON token approval failed");
      return { kind: "approval_confirmed" as const };
    }
  }
  const tx = action.transaction;
  const from = tronAddress(web, String(tx.from));
  if (from !== wallet.address) throw new Error("Symbiosis TRON sender does not match the connected wallet");
  const data = String(tx.data).replace(/^0x/, "");
  const selector = typeof tx.functionSelector === "string" ? tx.functionSelector : "";
  const feeLimit = Number(tx.feeLimit);
  if (!selector || !/^[a-f\d]+$/i.test(data) || !Number.isSafeInteger(feeLimit) || feeLimit <= 0) throw new Error("Invalid Symbiosis TRON transaction");
  // Symbiosis supplies ABI argument bytes alongside the function selector.
  // Pass those bytes unchanged; do not decode/re-encode provider calldata.
  const built = await web.transactionBuilder.triggerSmartContract(tronAddress(web, String(tx.to)), selector, {
    feeLimit, callValue: String(tx.value ?? "0"), rawParameter: data,
  }, [], from);
  if (!built.result.result) throw new Error("Unable to build Symbiosis TRON transaction");
  return { reference: await broadcastTron(web, built.transaction), kind: "transaction_hash" as const };
}

/** Restore only sessions authorized in a previous visit, without wallet prompts. */
export async function restoreWallet(family: WalletFamily): Promise<ConnectedWallet | null> {
  if (family === "near") {
    const selector = await nearSelector();
    const account = selector.store.getState().accounts.find(item => item.active);
    return account ? { family, address: account.accountId, selector } : null;
  }
  if (family === "tron") {
    const provider = tronProvider();
    const web = provider.tronWeb;
    if (!web || !web.ready || !web.defaultAddress.base58) return null;
    return { family, address: web.defaultAddress.base58, chainId: TRON_CHAIN_ID, provider };
  }
  const { config } = env.PUBLIC_REOWN_PROJECT_ID?.trim() ? await appKit() : await injectedApp();
  const { reconnect, getAccount } = await import("@wagmi/core");
  await reconnect(config as Parameters<typeof reconnect>[0]);
  const account = getAccount(config as Parameters<typeof getAccount>[0]);
  return account.isConnected && account.address && account.chainId ? { family, address: account.address, chainId: account.chainId, config } : null;
}

export async function observeWallet(wallet: ConnectedWallet, changed: (value: ConnectedWallet | null) => void): Promise<() => void> {
  if (wallet.family === "near") {
    const subscription = wallet.selector.store.observable.subscribe(state => {
      const account = state.accounts.find(item => item.active);
      changed(account ? { ...wallet, address: account.accountId } : null);
    });
    return () => subscription.unsubscribe();
  }
  if (wallet.family === "evm") {
    const { watchAccount } = await import("@wagmi/core");
    return watchAccount(wallet.config as Parameters<typeof watchAccount>[0], { onChange: account => {
      changed(account.isConnected && account.address && account.chainId ? { ...wallet, address: account.address, chainId: account.chainId } : null);
    } });
  }
  const update = () => {
    const web = wallet.provider.tronWeb;
    // Invalidate on network changes instead of silently signing on another node.
    changed(web && web.ready && web.defaultAddress.base58 && new URL(web.fullNode.host).hostname === "api.trongrid.io"
      ? { ...wallet, address: web.defaultAddress.base58 } : null);
  };
  const events = ["accountsChanged", "chainChanged", "connect", "disconnect"];
  for (const event of events) wallet.provider.on?.(event, update);
  const legacy = (event: MessageEvent) => {
    if (event.source === window && ["setAccount", "setNode", "disconnect", "connect"].includes(event.data?.message?.action)) update();
  };
  window.addEventListener("message", legacy);
  // Legacy wallets do not emit lock events; inspect their authorized state too.
  const timer = window.setInterval(update, 2_000);
  return () => {
    for (const event of events) wallet.provider.removeListener?.(event, update);
    window.removeEventListener("message", legacy);
    window.clearInterval(timer);
  };
}

export async function disconnectWallet(wallet: ConnectedWallet): Promise<void> {
  if (wallet.family === "near") await (await wallet.selector.wallet()).signOut();
  if (wallet.family === "evm") {
    const { disconnect } = await import("@wagmi/core");
    await disconnect(wallet.config as Parameters<typeof disconnect>[0]);
  }
  // TronLink has no portable permission-revocation API; disconnect locally.
}
