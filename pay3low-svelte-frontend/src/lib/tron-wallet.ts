export const TRON_CHAIN_ID = 728126428;
export const TRON_CHAIN_HEX = "0x2b6653dc";

type Transaction = Record<string, unknown>;
interface ContractMethod {
  call(): Promise<string | { toString(): string }>;
  send(options: { feeLimit: number; shouldPollResponse?: boolean; keepTxID?: boolean }): Promise<string | [string, unknown]>;
}
export interface TronWeb {
  ready: boolean;
  defaultAddress: { base58: string; hex?: string };
  fullNode: { host: string };
  isAddress(address: string): boolean;
  address: { toHex(address: string): string; fromHex(address: string): string };
  contract(abi: readonly unknown[], address: string): Promise<{
    balanceOf(owner: string): ContractMethod;
    allowance(owner: string, spender: string): ContractMethod;
    approve(spender: string, amount: string): ContractMethod;
    transfer(recipient: string, amount: string): ContractMethod;
  }>;
  transactionBuilder: {
    sendTrx(to: string, amount: string, from: string): Promise<Transaction>;
    triggerSmartContract(to: string, selector: string, options: { feeLimit: number; callValue: string; rawParameter: string }, parameters: unknown[], from: string): Promise<{ result: { result: boolean }; transaction: Transaction }>;
  };
  trx: {
    getBalance(address: string): Promise<number | string>;
    sign(transaction: Transaction): Promise<Transaction>;
    sendRawTransaction(transaction: Transaction): Promise<{ result: boolean; txid?: string; message?: string }>;
  };
}

export interface TronProvider {
  tronWeb: TronWeb | false;
  request(args: { method: string; params?: unknown }): Promise<unknown>;
  on?(event: string, listener: (value: unknown) => void): void;
  removeListener?(event: string, listener: (value: unknown) => void): void;
}

export function tronProvider(): TronProvider {
  const browser = window as Window & { tron?: TronProvider; tronLink?: TronProvider; tronWeb?: TronWeb };
  const provider = browser.tron ?? browser.tronLink;
  if (!provider) throw new Error("TronLink is not installed. Install TronLink to connect a TRON wallet.");
  // Older extensions publish the authorized instance directly on window.
  if (!provider.tronWeb && browser.tronWeb) provider.tronWeb = browser.tronWeb;
  return provider;
}

export function tronAddress(web: TronWeb, address: string): string {
  const normalized = /^0x[\da-f]{40}$/i.test(address) ? `41${address.slice(2)}` : address;
  const base58 = /^(?:41)[\da-f]{40}$/i.test(normalized) ? web.address.fromHex(normalized) : normalized;
  if (!web.isAddress(base58)) throw new Error("Invalid TRON address");
  return base58;
}

export async function connectTronProvider(): Promise<TronProvider> {
  const provider = tronProvider();
  if (!provider.tronWeb || !provider.tronWeb.ready) {
    try {
      await provider.request({ method: "eth_requestAccounts" });
    } catch (error) {
      const code = (error as { code?: number }).code;
      if (code !== 4200 && code !== -32601) throw error;
      const result = await provider.request({ method: "tron_requestAccounts" });
      if (result && typeof result === "object" && "code" in result && result.code !== 200) throw new Error("TRON wallet connection was cancelled");
    }
  }
  if (!provider.tronWeb || !provider.tronWeb.ready || !provider.tronWeb.defaultAddress.base58) throw new Error("Unlock TronLink and authorize this website");
  await ensureTronMainnet(provider);
  return provider;
}

export async function ensureTronMainnet(provider: TronProvider): Promise<TronWeb> {
  let web = provider.tronWeb;
  if (!web || !web.ready) throw new Error("TRON wallet is locked or disconnected");
  // Legacy TronLink has no chain query API; its bound node identifies mainnet.
  if (new URL(web.fullNode.host).hostname !== "api.trongrid.io") {
    await provider.request({ method: "wallet_switchEthereumChain", params: [{ chainId: TRON_CHAIN_HEX }] });
    web = provider.tronWeb;
    if (!web || !web.ready || new URL(web.fullNode.host).hostname !== "api.trongrid.io") throw new Error("Switch TronLink to TRON mainnet");
  }
  return web;
}

export async function broadcastTron(web: TronWeb, transaction: Transaction): Promise<string> {
  const signed = await web.trx.sign(transaction);
  const result = await web.trx.sendRawTransaction(signed);
  if (!result.result || !result.txid) throw new Error("TRON transaction broadcast failed");
  return result.txid;
}
