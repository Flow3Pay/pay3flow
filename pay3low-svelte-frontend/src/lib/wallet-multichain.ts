import { env } from "$env/dynamic/public";
import type { RouteExecutionAction } from "$lib/exchange";
import { Buffer } from "buffer";

type Hex = `0x${string}`;
type AddressLike = { toBase58(): string };

interface SolanaProvider {
  publicKey?: AddressLike | null;
  isConnected?: boolean;
  connect(): Promise<{ publicKey?: AddressLike }>;
  disconnect?(): Promise<void>;
  signTransaction(transaction: unknown): Promise<{ serialize(): Uint8Array }>;
  on?(event: string, listener: (...args: unknown[]) => void): void;
  off?(event: string, listener: (...args: unknown[]) => void): void;
}

interface AptosAccount { address: string }
interface AptosProvider {
  connect(): Promise<AptosAccount>;
  account(): Promise<AptosAccount>;
  disconnect?(): Promise<void>;
  network?(): Promise<{ chainId?: number | string; name?: string } | string>;
  signAndSubmitTransaction(input: {
    data: { function: string; functionArguments: [string, string, string]; typeArguments: [] };
  }): Promise<{ hash: string }>;
}

interface StandardAccount { address: string; chains?: string[] }
interface SuiWallet {
  name: string;
  chains: string[];
  accounts: StandardAccount[];
  features: Record<string, unknown>;
  on?(event: "change", listener: () => void): () => void;
}
interface SuiClient {
  getBalance(input: { owner: string; coinType: string }): Promise<{ totalBalance: string }>;
  getCoins(input: { owner: string; coinType: string; limit?: number; cursor?: string | null }): Promise<{
    data: { coinObjectId: string; balance: string }[];
    hasNextPage: boolean;
    nextCursor?: string | null;
  }>;
}
interface SuiTransaction {
  setSender(sender: string): void;
  setGasBudget(amount: number): void;
  mergeCoins(target: unknown, sources: unknown[]): unknown;
  splitCoins(coin: unknown, amounts: bigint[]): unknown[];
  object(id: string): unknown;
  transferObjects(objects: unknown[], address: string): void;
}
interface TonAccount { address: string; chain: string }
interface TonConnect {
  connected: boolean;
  account: TonAccount | null;
  connectionRestored: Promise<boolean>;
  setConnectionNetwork(network?: string): void;
  onStatusChange(callback: (wallet: { account?: TonAccount } | null) => void): () => void;
  openModal(): Promise<void>;
  sendTransaction(input: {
    validUntil: number;
    network: string;
    from: string;
    messages: { address: string; amount: string; payload?: string }[];
  }): Promise<{ boc: string }>;
  disconnect(): Promise<void>;
}

function requiredField(value: string | undefined, field: string): string {
  if (!value?.trim()) throw new Error(`Payment instructions are missing ${field}`);
  return value;
}

function requiredDecimals(value: number | undefined): number {
  if (!Number.isInteger(value) || value! < 0 || value! > 255) throw new Error("Payment instructions have invalid token precision");
  return value!;
}

function canonicalAptosAddress(address: string): string {
  const value = address.replace(/^0x/i, "");
  if (!/^[0-9a-f]{1,64}$/i.test(value)) throw new Error("Aptos wallet returned an invalid address");
  return `0x${value.padStart(64, "0").toLowerCase()}`;
}

function isAptosMainnet(network: Awaited<ReturnType<NonNullable<AptosProvider["network"]>>> | undefined): boolean {
  if (typeof network === "string") return network.toLowerCase() === "mainnet";
  return network?.chainId === 1 || network?.chainId === "1" || network?.name?.toLowerCase() === "mainnet";
}

export type ExtraWallet =
  | { family: "solana"; address: string; provider: SolanaProvider }
  | { family: "sui"; address: string; wallet: SuiWallet; account: StandardAccount }
  | { family: "aptos"; address: string; provider: AptosProvider }
  | { family: "ton"; address: string; chainId: -239; ui: TonConnect };

let tonUiPromise: Promise<TonConnect> | null = null;
const suiClients = new Map<string, Promise<SuiClient>>();

function record(value: unknown): Record<string, unknown> | null {
  return value !== null && typeof value === "object" ? value as Record<string, unknown> : null;
}

function rpcUrl(value: string | undefined, network: string): string {
  const url = value?.trim();
  if (!url) throw new Error(`${network} RPC URL is not configured`);
  const parsed = new URL(url);
  if (parsed.protocol !== "https:" && parsed.hostname !== "localhost" && parsed.hostname !== "127.0.0.1") {
    throw new Error(`${network} RPC URL must use HTTPS`);
  }
  return url;
}

function solanaProvider(): SolanaProvider {
  const provider = (window as Window & { solana?: unknown }).solana;
  const candidate = record(provider);
  if (!candidate || typeof candidate.connect !== "function" || typeof candidate.signTransaction !== "function") {
    throw new Error("Install a Solana browser wallet such as Phantom");
  }
  return provider as SolanaProvider;
}

function aptosProvider(): AptosProvider {
  const provider = (window as Window & { aptos?: unknown }).aptos;
  const candidate = record(provider);
  if (!candidate || typeof candidate.connect !== "function" || typeof candidate.account !== "function"
    || typeof candidate.signAndSubmitTransaction !== "function") {
    throw new Error("Install an Aptos browser wallet such as Petra");
  }
  return provider as AptosProvider;
}

async function tonUi(): Promise<TonConnect> {
  const manifestUrl = env.PUBLIC_TONCONNECT_MANIFEST_URL?.trim();
  if (!manifestUrl) throw new Error("PUBLIC_TONCONNECT_MANIFEST_URL is not configured");
  if (!tonUiPromise) {
    tonUiPromise = import("@tonconnect/ui").then(({ TonConnectUI }) => {
      const ui = new TonConnectUI({ manifestUrl });
      ui.setConnectionNetwork("-239");
      return ui as unknown as TonConnect;
    }).catch(error => {
      tonUiPromise = null;
      throw error;
    });
  }
  return tonUiPromise;
}

function normalizedTonAddress(address: string): string {
  if (address.includes(":")) {
    const [workchain, hash] = address.split(":");
    if (!/^-?\d+$/.test(workchain) || !/^[a-f\d]{64}$/i.test(hash ?? "")) throw new Error("Invalid TON wallet address");
    return address;
  }
  return address;
}

async function connectSui(): Promise<ExtraWallet> {
  const { getWallets } = await import("@wallet-standard/app");
  const wallets = getWallets().get().filter(wallet => wallet.chains.includes("sui:mainnet"));
  if (!wallets.length) throw new Error("No Sui mainnet wallet was found");
  if (wallets.length > 1) throw new Error("More than one Sui wallet is available; leave one Sui wallet enabled and retry");
  const wallet = wallets[0] as unknown as SuiWallet;
  const connect = record(wallet.features["standard:connect"])?.connect;
  if (typeof connect !== "function") throw new Error("The selected Sui wallet does not support connection");
  const result = await (connect as () => Promise<{ accounts?: StandardAccount[] }>)();
  const account = result.accounts?.[0] ?? wallet.accounts[0];
  if (!account?.address || !/^0x[0-9a-f]{64}$/i.test(account.address)) {
    throw new Error("The Sui wallet returned an invalid mainnet address");
  }
  return { family: "sui", address: account.address, wallet, account };
}

export async function connectExtraWallet(network: string): Promise<ExtraWallet> {
  if (network === "solana") {
    const provider = solanaProvider();
    const result = await provider.connect();
    const address = result.publicKey?.toBase58() ?? provider.publicKey?.toBase58();
    if (!address) throw new Error("Solana wallet did not return an account");
    return { family: "solana", address, provider };
  }
  if (network === "sui") return connectSui();
  if (network === "aptos") {
    const provider = aptosProvider();
    const account = await provider.connect();
    const chain = await provider.network?.();
    if (!isAptosMainnet(chain)) throw new Error("Switch the Aptos wallet to mainnet and reconnect");
    return { family: "aptos", address: canonicalAptosAddress(account.address), provider };
  }
  if (network === "ton") {
    const ui = await tonUi();
    if (ui.connected && ui.account?.chain === "-239") {
      return { family: "ton", address: normalizedTonAddress(ui.account.address), chainId: -239, ui };
    }
    await new Promise<void>((resolve, reject) => {
      let unsubscribe = () => {};
      const timeout = window.setTimeout(() => {
        unsubscribe();
        reject(new Error("TON wallet connection timed out"));
      }, 120_000);
      unsubscribe = ui.onStatusChange(wallet => {
        if (!wallet?.account) return;
        window.clearTimeout(timeout);
        unsubscribe();
        if (wallet.account.chain !== "-239") {
          reject(new Error("Connect a TON mainnet wallet"));
          return;
        }
        resolve();
      });
      void ui.openModal().catch(error => {
        window.clearTimeout(timeout);
        unsubscribe();
        reject(error);
      });
    });
    const account = ui.account;
    if (!account || account.chain !== "-239") throw new Error("TON wallet did not connect to mainnet");
    return { family: "ton", address: normalizedTonAddress(account.address), chainId: -239, ui };
  }
  throw new Error(`Wallet execution is not available for ${network}`);
}

export async function restoreExtraWallet(family: ExtraWallet["family"]): Promise<ExtraWallet | null> {
  if (family === "solana") {
    const provider = solanaProvider();
    const address = provider.isConnected ? provider.publicKey?.toBase58() : undefined;
    return address ? { family, address, provider } : null;
  }
  if (family === "sui") {
    const { getWallets } = await import("@wallet-standard/app");
    const wallets = getWallets().get().filter(wallet => wallet.chains.includes("sui:mainnet"));
    if (wallets.length !== 1) return null;
    const wallet = wallets[0] as unknown as SuiWallet;
    const account = wallet.accounts.find(item => item.chains?.includes("sui:mainnet")) ?? wallet.accounts[0];
    return account?.address ? { family, address: account.address, wallet, account } : null;
  }
  if (family === "aptos") {
    const provider = aptosProvider();
    const account = await provider.account();
    const chain = await provider.network?.();
    return account.address && isAptosMainnet(chain)
      ? { family, address: canonicalAptosAddress(account.address), provider }
      : null;
  }
  const ui = await tonUi();
  await ui.connectionRestored;
  return ui.connected && ui.account?.chain === "-239"
    ? { family, address: normalizedTonAddress(ui.account.address), chainId: -239, ui }
    : null;
}

export async function observeExtraWallet(wallet: ExtraWallet, changed: (wallet: ExtraWallet | null) => void): Promise<() => void> {
  if (wallet.family === "ton") {
    return wallet.ui.onStatusChange(account => {
      changed(account?.account?.chain === "-239"
        ? { ...wallet, address: normalizedTonAddress(account.account.address) }
        : null);
    });
  }
  if (wallet.family === "sui" && wallet.wallet.on) {
    return wallet.wallet.on("change", () => {
      const account = wallet.wallet.accounts.find(value => value.address === wallet.address);
      changed(account ? { ...wallet, account } : null);
    });
  }
  let active = true;
  const timer = window.setInterval(() => {
    void restoreExtraWallet(wallet.family).then(current => {
      if (!active) return;
      changed(current?.address === wallet.address ? current : null);
    }).catch(() => { if (active) changed(null); });
  }, 2_000);
  return () => { active = false; window.clearInterval(timer); };
}

export async function disconnectExtraWallet(wallet: ExtraWallet): Promise<void> {
  if (wallet.family === "ton") await wallet.ui.disconnect();
  else if (wallet.family === "solana") await wallet.provider.disconnect?.();
  else if (wallet.family === "aptos") await wallet.provider.disconnect?.();
  else {
    const disconnect = record(wallet.wallet.features["standard:disconnect"])?.disconnect;
    if (typeof disconnect === "function") await (disconnect as () => Promise<void>)();
  }
}

async function getSuiClient(): Promise<SuiClient> {
  const url = rpcUrl(env.PUBLIC_SUI_RPC_URL, "Sui");
  let client = suiClients.get(url);
  if (!client) {
    client = import("@mysten/sui/client").then(({ SuiClient }) => new SuiClient({ url }));
    suiClients.set(url, client);
  }
  return client;
}

async function getSolanaConnection() {
  const endpoint = rpcUrl(env.PUBLIC_SOLANA_RPC_URL, "Solana");
  const { Connection } = await import("@solana/web3.js");
  return new Connection(endpoint, "confirmed");
}

async function getAptosStoreBalance(owner: string, metadata: string): Promise<bigint> {
  const base = rpcUrl(env.PUBLIC_APTOS_RPC_URL, "Aptos").replace(/\/+$/, "");
  const resourcesResponse = await fetch(`${base}/accounts/${encodeURIComponent(owner)}/resources`);
  if (!resourcesResponse.ok) throw new Error("Unable to read Aptos account resources");
  const resources = await resourcesResponse.json() as { type?: string; data?: { metadata?: string; balance?: string } }[];
  const store = resources.find(resource => resource.type?.includes("fungible_asset::FungibleStore")
    && resource.data?.metadata?.toLowerCase() === metadata.toLowerCase());
  return BigInt(store?.data?.balance ?? "0");
}

interface JettonBalance { jetton?: { address?: string }; wallet_address?: { address?: string }; balance?: string }
async function tonJettonBalance(wallet: string, master: string): Promise<{ amount: bigint; walletAddress: string }> {
  const base = rpcUrl(env.PUBLIC_TON_API_URL, "TON API").replace(/\/+$/, "");
  const response = await fetch(`${base}/v2/accounts/${encodeURIComponent(wallet)}/jettons`);
  if (!response.ok) throw new Error("Unable to read TON jetton balance");
  const result = await response.json() as { balances?: JettonBalance[] };
  const { Address } = await import("@ton/core");
  const canonicalMaster = Address.parse(master).toRawString().toLowerCase();
  const token = result.balances?.find(item => {
    if (!item.jetton?.address) return false;
    try { return Address.parse(item.jetton.address).toRawString().toLowerCase() === canonicalMaster; }
    catch { return false; }
  });
  return { amount: BigInt(token?.balance ?? "0"), walletAddress: token?.wallet_address?.address ?? "" };
}

export async function hasExtraExecutionFunds(action: Extract<RouteExecutionAction, { kind: "near_deposit" }>, wallet: ExtraWallet, required: bigint): Promise<boolean> {
  const tokenContract = requiredField(action.token_contract, "the token contract");
  if (wallet.family === "solana") {
    const [{ PublicKey }, spl, connection] = await Promise.all([
      import("@solana/web3.js"), import("@solana/spl-token"), getSolanaConnection(),
    ]);
    const mint = new PublicKey(tokenContract);
    const account = await spl.getAssociatedTokenAddress(mint, new PublicKey(wallet.address));
    try { return (await spl.getAccount(connection, account)).amount >= required; }
    catch (error) {
      if (error instanceof spl.TokenAccountNotFoundError) return false;
      throw error;
    }
  }
  if (wallet.family === "sui") {
    const client = await getSuiClient();
    const balance = await client.getBalance({ owner: wallet.address, coinType: tokenContract });
    return BigInt(balance.totalBalance) >= required;
  }
  if (wallet.family === "aptos") {
    return await getAptosStoreBalance(wallet.address, tokenContract) >= required;
  }
  const jetton = await tonJettonBalance(wallet.address, tokenContract);
  return jetton.amount >= required && Boolean(jetton.walletAddress);
}

function memoCell(text: string, beginCell: (typeof import("@ton/core"))["beginCell"]) {
  return beginCell().storeUint(0, 32).storeStringTail(text).endCell();
}

async function executeSolana(action: Extract<RouteExecutionAction, { kind: "near_deposit" }>, wallet: Extract<ExtraWallet, { family: "solana" }>, amount: bigint) {
  if (action.deposit_memo && action.deposit_memo.length > 566) throw new Error("Solana deposit memo is too long");
  const [{ PublicKey, Transaction, TransactionInstruction }, spl, connection] = await Promise.all([
    import("@solana/web3.js"), import("@solana/spl-token"), getSolanaConnection(),
  ]);
  const owner = new PublicKey(wallet.address);
  const mint = new PublicKey(requiredField(action.token_contract, "the token contract"));
  const destination = new PublicKey(requiredField(action.deposit_address, "the deposit address"));
  const sourceAta = await spl.getAssociatedTokenAddress(mint, owner);
  const destinationAta = await spl.getAssociatedTokenAddress(mint, destination, true);
  const transaction = new Transaction();
  transaction.add(spl.createAssociatedTokenAccountIdempotentInstruction(owner, destinationAta, destination, mint));
  transaction.add(spl.createTransferCheckedInstruction(sourceAta, mint, destinationAta, owner, amount, requiredDecimals(action.decimals)));
  if (action.deposit_memo) {
    transaction.add(new TransactionInstruction({
      keys: [],
      programId: new PublicKey("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr"),
      data: Buffer.from(action.deposit_memo, "utf8"),
    }));
  }
  const latest = await connection.getLatestBlockhash("confirmed");
  transaction.feePayer = owner;
  transaction.recentBlockhash = latest.blockhash;
  const signed = await wallet.provider.signTransaction(transaction);
  const reference = await connection.sendRawTransaction(signed.serialize(), { preflightCommitment: "confirmed" });
  await connection.confirmTransaction({ signature: reference, ...latest }, "confirmed");
  return reference;
}

async function executeSui(action: Extract<RouteExecutionAction, { kind: "near_deposit" }>, wallet: Extract<ExtraWallet, { family: "sui" }>, amount: bigint) {
  if (action.deposit_memo) throw new Error("Sui deposits do not support a payment memo");
  const { Transaction } = await import("@mysten/sui/transactions");
  const client = await getSuiClient();
  const coinType = requiredField(action.token_contract, "the token type");
  let remaining = amount;
  const selected: { coinObjectId: string; balance: bigint }[] = [];
  let cursor: string | null | undefined;
  let pageHasNext = true;
  while (remaining > 0n && pageHasNext) {
    const page = await client.getCoins({ owner: wallet.address, coinType, limit: 50, cursor });
    for (const coin of page.data) {
      selected.push({ coinObjectId: coin.coinObjectId, balance: BigInt(coin.balance) });
      remaining -= BigInt(coin.balance);
      if (remaining <= 0n) break;
    }
    pageHasNext = page.hasNextPage && Boolean(page.nextCursor);
    cursor = page.nextCursor;
  }
  if (remaining > 0n || selected.length === 0) throw new Error("Insufficient Sui token balance");
  const transaction = new Transaction() as unknown as SuiTransaction;
  transaction.setSender(wallet.address);
  transaction.setGasBudget(20_000_000);
  const coins = selected.map(coin => transaction.object(coin.coinObjectId));
  if (coins.length > 1) transaction.mergeCoins(coins[0], coins.slice(1));
  const [payment] = transaction.splitCoins(coins[0], [amount]);
  transaction.transferObjects([payment], action.deposit_address);
  const features = record(wallet.wallet.features);
  const feature = record(features?.["sui:signAndExecuteTransaction"])
    ?? record(features?.["sui:signAndExecuteTransactionBlock"]);
  const method = feature?.signAndExecuteTransaction ?? feature?.signAndExecuteTransactionBlock;
  if (typeof method !== "function") throw new Error("The connected Sui wallet cannot submit transactions");
  const response = await (method as (input: Record<string, unknown>) => Promise<{ digest?: string }>)({
    account: wallet.account,
    chain: "sui:mainnet",
    transaction,
    transactionBlock: transaction,
    options: { showEffects: true },
  });
  if (!response.digest) throw new Error("Sui wallet did not return a transaction digest");
  return response.digest;
}

async function executeAptos(action: Extract<RouteExecutionAction, { kind: "near_deposit" }>, wallet: Extract<ExtraWallet, { family: "aptos" }>, amount: bigint) {
  if (action.deposit_memo) throw new Error("Aptos fungible-asset deposits do not support a payment memo");
  const result = await wallet.provider.signAndSubmitTransaction({
    data: {
      function: "0x1::primary_fungible_store::transfer",
      functionArguments: [requiredField(action.token_contract, "the fungible asset"), requiredField(action.deposit_address, "the deposit address"), amount.toString()],
      typeArguments: [],
    },
  });
  if (!/^0x[0-9a-f]{64}$/i.test(result.hash)) throw new Error("Aptos wallet returned an invalid transaction hash");
  return result.hash;
}

async function executeTon(action: Extract<RouteExecutionAction, { kind: "near_deposit" }>, wallet: Extract<ExtraWallet, { family: "ton" }>, amount: bigint) {
  const jetton = await tonJettonBalance(wallet.address, requiredField(action.token_contract, "the jetton master"));
  if (!jetton.walletAddress) throw new Error("TON jetton wallet is not available for this token");
  const { Address, Cell, beginCell } = await import("@ton/core");
  const user = Address.parse(wallet.address);
  const body = beginCell()
    .storeUint(0x0f8a7ea5, 32)
    .storeUint(0, 64)
    .storeCoins(amount)
    .storeAddress(Address.parse(action.deposit_address))
    .storeAddress(user)
    .storeBit(0)
    .storeCoins(action.deposit_memo ? 1n : 0n)
    .storeBit(action.deposit_memo ? 1 : 0);
  if (action.deposit_memo) body.storeRef(memoCell(action.deposit_memo, beginCell));
  const payload = body.endCell().toBoc().toString("base64");
  const response = await wallet.ui.sendTransaction({
    validUntil: Math.floor(Date.now() / 1000) + 300,
    network: "-239",
    from: user.toString({ bounceable: false, urlSafe: true }),
    messages: [{ address: jetton.walletAddress, amount: "100000000", payload }],
  });
  const cell = Cell.fromBoc(Buffer.from(response.boc, "base64"))[0];
  if (!cell) throw new Error("TON wallet returned an invalid transaction payload");
  return cell.hash().toString("hex");
}

export async function executeExtraDeposit(action: Extract<RouteExecutionAction, { kind: "near_deposit" }>, wallet: ExtraWallet, amount: bigint): Promise<string> {
  if (wallet.family === "solana") return executeSolana(action, wallet, amount);
  if (wallet.family === "sui") return executeSui(action, wallet, amount);
  if (wallet.family === "aptos") return executeAptos(action, wallet, amount);
  return executeTon(action, wallet, amount);
}

export async function assertExtraWalletCurrent(wallet: ExtraWallet): Promise<void> {
  const current = await restoreExtraWallet(wallet.family);
  if (!current || current.address.toLowerCase() !== wallet.address.toLowerCase()) {
    throw new Error(`${wallet.family} wallet account changed or disconnected`);
  }
  if (wallet.family === "aptos") {
    const chain = await wallet.provider.network?.();
    if (!isAptosMainnet(chain)) throw new Error("Aptos wallet is not on mainnet");
  }
}

export function isExtraNetwork(network: string): network is ExtraWallet["family"] {
  return network === "solana" || network === "sui" || network === "aptos" || network === "ton";
}

export async function validateExtraRecipient(network: string, address: string): Promise<void> {
  if (network === "solana") {
    const { isSolanaAddress } = await import("./wallet-address");
    if (!isSolanaAddress(address)) throw new Error("Invalid Solana recipient address");
  } else if (network === "sui") {
    if (!/^0x[0-9a-f]{64}$/i.test(address)) throw new Error("Invalid Sui recipient address");
  } else if (network === "aptos") {
    try { canonicalAptosAddress(address); }
    catch { throw new Error("Invalid Aptos recipient address"); }
  } else if (network === "ton") {
    try {
      const { Address } = await import("@ton/core");
      Address.parse(address);
    } catch {
      throw new Error("Invalid TON recipient address");
    }
  } else {
    throw new Error(`Recipient validation is unavailable for ${network}`);
  }
}
