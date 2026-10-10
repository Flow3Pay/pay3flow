import { apiUrl } from "$lib/api";
import { connectWallet } from "$lib/wallet-session";

export type Direction = "buy" | "sell";
export interface Proposal { id: string; name?: string; publishes: { resourceConformsTo: string }; reciprocal: { resourceConformsTo: string } }
export interface Terms {
  demo: boolean;
  direction: Direction; input: string; output: string; input_units: string; output_units: string;
  customer_actor: string; desk_actor: string; quote_by: string; pay_by: string; payout_by: string;
  network_costs: string; ever_receiving_cost_units: string; refund_policy: string;
  fee_policy: { fee_units: string; basis_units: string };
  proposal: Proposal;
}
export interface Trade { id: string; rfq_id: string; state: string; terms: Terms; offer: unknown; decision: unknown }
export interface Rfq { id: string; direction: Direction; input: string; actor: string; state: string }
export interface Attempt { id: string; kind: string; state: string; instructions: Instructions; reference: string | null }
interface Instructions {
  leg: { chain: string; sender: string; recipient: string; amount: string; transfer_amount: string };
  call: { family: string; from?: string; to?: string; data?: string; value?: string; nonce?: string; sender?: string; recipient?: string; amount?: string; bounce?: boolean };
}
export interface Config { enabled: boolean; available: boolean; demo: boolean; desk: string; desk_name: string; ever_resource: string; usdt_resource: string; counterparty_risk: string; support_owner: string }
export interface Session { token: string; actor: string; desk: boolean }
export async function request<T>(path: string, token = "", body?: unknown, key?: string): Promise<T> {
  const response = await fetch(apiUrl(`/api/otc${path}`), {
    method: body === undefined ? "GET" : "POST",
    headers: { ...(token ? { Authorization: `Bearer ${token}` } : {}), ...(body !== undefined ? { "Content-Type": "application/json" } : {}), ...(key ? { "Idempotency-Key": key } : {}) },
    body: body === undefined ? undefined : JSON.stringify(body), cache: "no-store"
  });
  const result = await response.json();
  if (!response.ok) throw new Error(result.error || `OTC request failed (${response.status})`);
  return result;
}
export function decimal(units: string, precision: number): string {
  const amount = BigInt(units), scale = 10n ** BigInt(precision);
  const fraction = (amount % scale).toString().padStart(precision, "0").replace(/0+$/, "");
  return `${amount / scale}${fraction ? `.${fraction}` : ""}`;
}
export function rate(terms: Terms): string {
  const ever = BigInt(terms.direction === "buy" ? terms.output_units : terms.input_units);
  const usdt = BigInt(terms.fee_policy.basis_units);
  return decimal((usdt * 1_000_000_000n / ever).toString(), 6);
}
export async function bindWallet(token: string, chain: "ethereum" | "everscale") {
  const wallet = await connectWallet(chain);
  const challenge = await request<{ id: string; message: string; everscale_data: string }>("/wallet/challenge", token, { chain, address: wallet.address });
  let signature: string, public_key: string | undefined;
  if (wallet.family === "evm") {
    if (wallet.chainId !== 1) throw new Error("Switch to Ethereum mainnet.");
    const { getWalletClient } = await import("@wagmi/core");
    const client = await getWalletClient(wallet.config as Parameters<typeof getWalletClient>[0], { chainId: 1 });
    signature = await client.signMessage({ account: wallet.address, message: challenge.message });
  } else if (wallet.family === "everscale") {
    const state = await wallet.provider.request({ method: "getProviderState" });
    public_key = state.permissions.accountInteraction?.publicKey;
    if (!public_key) throw new Error("EVER Wallet did not expose the account public key.");
    const signed = await wallet.provider.request({ method: "signData", params: { publicKey: public_key, data: challenge.everscale_data, withSignatureId: false } });
    signature = signed.signature;
  } else throw new Error("Unsupported wallet.");
  return request<{ address: string }>("/wallet/proof", token, { challenge_id: challenge.id, signature, public_key });
}
/** Called once after a durable handoff; an exception leaves the attempt unknown. */
export async function sendAttempt(token: string, attempt: Attempt): Promise<void> {
  const leg = attempt.instructions.leg;
  const wallet = await connectWallet(leg.chain);
  if (wallet.address.toLowerCase() !== leg.sender) throw new Error("Connected wallet differs from the booked sender. Reconcile this saved attempt before sending.");
  let reference: string;
  if (wallet.family === "evm") {
    if (wallet.chainId !== 1) throw new Error("Switch to Ethereum mainnet.");
    const { sendTransaction } = await import("@wagmi/core");
    const call = attempt.instructions.call;
    const nonce = BigInt(call.nonce!);
    if (nonce > BigInt(Number.MAX_SAFE_INTEGER)) throw new Error("Wallet nonce cannot be represented safely.");
    reference = await sendTransaction(wallet.config as Parameters<typeof sendTransaction>[0], { chainId: 1, account: wallet.address, to: call.to as `0x${string}`, data: call.data as `0x${string}`, value: 0n, nonce: Number(nonce) });
  } else if (wallet.family === "everscale") {
    const state = await wallet.provider.request({ method: "getProviderState" });
    if (state.selectedConnection !== "mainnet") throw new Error("Switch EVER Wallet to Everscale mainnet.");
    const result = await wallet.provider.request({ method: "sendMessage", params: { sender: leg.sender, recipient: leg.recipient, amount: leg.transfer_amount, bounce: true } });
    reference = result.transaction.id.hash;
  } else throw new Error("Unsupported wallet.");
  // Keep the original result available if the reference POST response is lost.
  sessionStorage.setItem(`pay3flow.otc.reference.${attempt.id}`, reference);
  await request(`/attempts/${attempt.id}/reference`, token, { reference });
}
