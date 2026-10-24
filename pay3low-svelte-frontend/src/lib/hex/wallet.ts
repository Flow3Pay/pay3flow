import { env } from "$env/dynamic/public";
import type { HotKit } from "@hot-labs/kit";
import type { OmniWallet, Intent } from "@hot-labs/kit/core";
import { CONTRACT, GNK, USDT, cancelMessage, orderMessage, type Draft, type Quote } from "./model";

export interface Wallet { account: string; address: string; name: string }
export type { Pending } from "./submission";
import { Submissions, ConfirmedFailure, withAccountLock } from "./submission";
let journal: Submissions | undefined;
type SDK = typeof import("./sdk-entry");
let sdkPromise: Promise<SDK> | undefined;
const sdk = () => sdkPromise ??= (import(/* @vite-ignore */ new URL("/vendor/hex-hot-kit.js",location.origin).href) as Promise<SDK>)
  .catch(error => { sdkPromise = undefined; throw error; });
let kitPromise: Promise<HotKit> | undefined;
export const configured = () => !!env.PUBLIC_HOT_KIT_API_KEY?.trim();
function submissions() {
  if (!journal) journal = new Submissions(localStorage, {
    publish: async signed => { const {api} = await sdk(); return bounded(api.publishIntents([signed],[])); },
    status: async hash => { const {api} = await sdk(); return bounded(api.getIntentsStatus(hash)); },
    verifyTransaction: async hash => {
      const {nearRpc} = await sdk();
      const result = await bounded(nearRpc.sendJsonRpc<Awaited<ReturnType<typeof nearRpc.waitTransactionResult>>>("EXPERIMENTAL_tx_status",{tx_hash:hash,sender_account_id:"intents.near",wait_until:"FINAL"}));
      if (!result.transaction || typeof result.status !== "object") throw new Error("Transaction confirmation is pending");
      if ("Failure" in result.status) throw new ConfirmedFailure("HEX transaction failed");
      try { nearRpc.parseReceipts(result); } catch (e) { throw new ConfirmedFailure(String(e)); }
    },
  });
  return journal;
}
export function pending(account: string) { return submissions().pending(account); }
async function bounded<T>(promise: Promise<T>, ms = 20_000): Promise<T> {
  let timer: ReturnType<typeof setTimeout>;
  try { return await Promise.race([promise, new Promise<never>((_,reject) => { timer = setTimeout(() => reject(new Error("HEX response timed out. Check the original submission.")), ms); })]); }
  finally { clearTimeout(timer!); }
}
export async function kit(): Promise<HotKit> {
  if (!configured()) throw new Error("HOT wallet integration is not configured");
  if (!kitPromise) {
    kitPromise = (async () => {
      const {HotKit,defaultConnectors,cosmos} = await sdk();
      const instance = new HotKit({ apiKey: env.PUBLIC_HOT_KIT_API_KEY!.trim(),
        connectors: [...defaultConnectors, cosmos({ chainId: "gonka-mainnet" })],
        walletConnect: { projectId: env.PUBLIC_REOWN_PROJECT_ID || undefined,
          metadata: { name: "Pay3Flow", description: "GNK trading on HEX", url: location.origin, icons: [`${location.origin}/icons/assets/gnk.jpg`] } } });
      // Connector initialization is asynchronous in HOT Kit 1.6.4.
      for (let attempt = 0; !instance.connectors.length; attempt++) {
        if (attempt >= 400) throw new Error("HOT wallet connectors did not initialize");
        await new Promise(resolve => setTimeout(resolve,50));
      }
      return instance;
    })().catch(error => { kitPromise = undefined; throw error; });
  }
  return kitPromise;
}
function describe(wallet: OmniWallet): Wallet { return { account: wallet.omniAddress, address: wallet.address, name: wallet.kit?.getWalletConnector(wallet.type)?.name || "Wallet" }; }
export async function watch(onChange: (wallets: Wallet[]) => void): Promise<() => void> {
  const instance = await kit();
  const update = () => onChange(instance.wallets.filter(w => w.omniAddress).map(describe));
  const off = [instance.onConnect(update),instance.onDisconnect(update)];
  update(); return () => off.forEach(fn => fn());
}
export async function connect(): Promise<Wallet> { return describe(await (await kit()).connect()); }
async function signer(account: string): Promise<{ instance: HotKit; wallet: OmniWallet }> {
  const instance = await kit(), wallet = instance.wallets.find(w => w.omniAddress === account);
  if (!wallet) throw new Error("The selected HEX wallet is no longer connected");
  return { instance, wallet };
}
export async function disconnect(account: string) { const {instance,wallet} = await signer(account); await instance.disconnect(wallet); }
export async function fund(action: "deposit" | "withdraw", token: string, account: string) {
  const {instance,wallet} = await signer(account);
  const { OmniToken, tokens, Recipient } = await sdk();
  const asset = token === GNK ? OmniToken.GONKA : OmniToken.USDT;
  // The bridge UI selects the source/destination network and reviews its fees.
  // Trading is a separate action and always gets a fresh simulation afterwards.
  if (action === "deposit") {
    await tokens.getTokens();
    const omni = tokens.get(asset), original = tokens.get(omni.originalAddress,omni.originalChain);
    if (!original) throw new Error("HOT has no deposit route for this asset");
    await instance.router.openBridge(instance,{mobileFullscreen:true,type:"exactOut",to:omni,from:original,
      sender:instance.wallets.find(w => w.type === original.type),recipient:Recipient.fromWallet(wallet)});
  }
  else await instance.withdraw(asset, undefined, { sender: wallet });
}
export async function profile() { await (await kit()).openProfile(); }

async function submit(account: string, intent: Intent): Promise<string | null> {
  const publish = () => submissions().submit(account,async () => {
    const {instance,wallet} = await signer(account);
    const signed = await instance.intentsBuilder(wallet).addIntent(intent).attachTimeout(120).sign();
    await signer(account); // A wallet change during signing prevents publication.
    return signed;
  });
  return withAccountLock(navigator.locks,account,publish);
}
export async function place(account: string, draft: Draft, quote: Quote, slippage: string) {
  return submit(account, { intent: "transfer", receiver_id: CONTRACT, min_gas: "200000000000000",
    tokens: { [draft.side === "buy" ? USDT : GNK]: draft.amount }, msg: JSON.stringify(orderMessage(draft,quote,slippage)) });
}
export async function cancel(account: string, hash: string) {
  return submit(account, { intent: "auth_call", contract_id: CONTRACT, attached_deposit: "0", min_gas: "200000000000000", msg: JSON.stringify(cancelMessage(hash)) });
}
export async function claim(account: string) {
  return submit(account, { intent: "auth_call", contract_id: CONTRACT, attached_deposit: "0", min_gas: "200000000000000", msg: JSON.stringify({action:"claim_balance"}) });
}
export async function reconcile(account: string): Promise<string | null> {
  await signer(account);
  return withAccountLock(navigator.locks,account,() => submissions().reconcile(account));
}
