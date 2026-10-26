import { apiUrl } from "$lib/api";
import { connectWallet } from "$lib/wallet-session";
import { evmNetworkFor, p2pEvmNetworkFor, walletNetworkName } from "./network";

export type Direction = "buy" | "sell";
export interface Proposal { id: string; name?: string; publishes: { resourceConformsTo: string }; reciprocal: { resourceConformsTo: string } }
export interface Terms {
  escrow?: import("./escrow").EscrowPolicy;
  demo: boolean;
  customer_ever?: string; customer_usdt?: string; desk_ever?: string; desk_usdt?: string;
  direction: Direction; input: string; output: string; input_units: string; output_units: string;
  customer_actor: string; desk_actor: string; quote_by: string; pay_by: string; payout_by: string;
  network_costs: string; ever_receiving_cost_units: string; refund_policy: string;
  fee_policy: { fee_units: string; basis_units: string };
  proposal: Proposal;
}
export interface Trade { created_at?: string; id: string; rfq_id: string; state: string; terms: Terms; offer: unknown; decision: unknown }
export interface Rfq { created_at?: string; id: string; direction: Direction; input: string; actor: string; state: string }
export interface Attempt { id: string; kind: string; state: string; instructions: Instructions; reference: string | null }
interface Instructions {
  escrow?: boolean; expires_at?: string; trade_id?: string; submit_path?: string;
  leg: { chain: string; sender: string; recipient: string; amount: string; transfer_amount: string };
  call: { family: string; chain_id?: number; from?: string; to?: string; data?: string; value?: string; nonce?: string; sender?: string; recipient?: string; amount?: string; bounce?: boolean };
}
export interface Config { public_offers_available?: boolean; public_offer_directions?: Direction[]; everscale_multisig_available?: boolean; p2p_available?: boolean; ethereum_chain_id?: number; usdt_contract?: string; usdt_network?: string; p2p_ethereum_chain_id?: number; p2p_usdt_contract?: string; p2p_usdt_network?: string; escrow?: import("./escrow").EscrowPolicy | null; enabled: boolean; wallet_login_available?: boolean; wallet_login_chains?: ("ethereum" | "everscale")[]; wallet_verification_days?: number; available: boolean; demo: boolean; desk: string; desk_name: string; ever_resource: string; usdt_resource: string; counterparty_risk: string; support_owner: string }
export type LoginChain = "ethereum" | "everscale";
export interface VerifiedWallet { chain: LoginChain; address: string; verified_at: string }
export interface Session { token: string; actor: string; desk: boolean; expires_at: string; signing_wallet: { chain: LoginChain; address: string } | null; wallets: VerifiedWallet[] }
let evmNetwork = evmNetworkFor(1);
let p2pNetwork = evmNetworkFor(1);
export function otcEvmNetwork() { return evmNetwork; }
/** Offers settle between peers on their own network, which may differ from the desk one. */
export function otcP2pNetwork() { return p2pNetwork; }
/** The backend chooses the OTC networks; the browser must follow its configuration. */
export function setOtcNetwork(config: Config) { evmNetwork = evmNetworkFor(config.ethereum_chain_id); p2pNetwork = p2pEvmNetworkFor(config); }
export function walletNetwork(chain: string, network = evmNetwork): string { return walletNetworkName(chain, network); }
export class OtcRequestError extends Error {
  constructor(message: string, readonly status: number) { super(message); this.name = "OtcRequestError"; }
}
export async function request<T>(path: string, token = "", body?: unknown, key?: string): Promise<T> {
  const response = await fetch(apiUrl(`/api/otc${path}`), {
    method: body === undefined ? "GET" : "POST",
    headers: { ...(token ? { Authorization: `Bearer ${token}` } : {}), ...(body !== undefined ? { "Content-Type": "application/json" } : {}), ...(key ? { "Idempotency-Key": key } : {}) },
    body: body === undefined ? undefined : JSON.stringify(body), cache: "no-store"
  });
  const result = await response.json();
  if (!response.ok) throw new OtcRequestError(result.error || `OTC request failed (${response.status})`, response.status);
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
interface WalletChallenge { id: string; message: string; everscale_data: string }
async function walletProof(wallet: Awaited<ReturnType<typeof connectWallet>>, challenge: WalletChallenge) {
  let signature: string, public_key: string | undefined;
  if (wallet.family === "evm") {
    if (wallet.chainId !== evmNetwork.chainId) throw new Error(`Switch to ${evmNetwork.label}.`);
    const { getWalletClient } = await import("@wagmi/core");
    const client = await getWalletClient(wallet.config as Parameters<typeof getWalletClient>[0], { chainId: evmNetwork.chainId });
    signature = await client.signMessage({ account: wallet.address, message: challenge.message });
  } else if (wallet.family === "everscale") {
    const state = await wallet.provider.request({ method: "getProviderState" });
    if (state.selectedConnection !== "mainnet" || state.permissions.accountInteraction?.address.toLowerCase() !== wallet.address.toLowerCase()) throw new Error("Wallet account or network changed. Please connect again.");
    public_key = state.permissions.accountInteraction?.publicKey;
    if (!public_key) throw new Error("EVER Wallet did not expose the account public key.");
    const signed = await wallet.provider.request({ method: "signData", params: { publicKey: public_key, data: challenge.everscale_data, withSignatureId: false } });
    signature = signed.signature;
  } else throw new Error("Unsupported wallet.");
  return { challenge_id: challenge.id, signature, public_key };
}
export async function signInWallet(chain: LoginChain, progress: (message: string) => void = () => {}) {
  progress("Connecting wallet…");
  const wallet = await connectWallet(walletNetwork(chain));
  const challenge = await request<WalletChallenge>("/session/challenge", "", { chain, address: wallet.address });
  progress("Confirm sign-in in your wallet…");
  const proof = await walletProof(wallet, challenge);
  progress("Signing in…");
  return request<Session>("/session/proof", "", proof);
}
export async function bindWallet(token: string, chain: LoginChain, progress: (message: string) => void = () => {}) {
  progress("Connecting wallet…");
  const wallet = await connectWallet(walletNetwork(chain));
  const challenge = await request<WalletChallenge>("/wallet/challenge", token, { chain, address: wallet.address });
  progress("Confirm wallet ownership…");
  const proof = await walletProof(wallet, challenge);
  progress("Verifying wallet…");
  return request<{ address: string }>("/wallet/proof", token, proof);
}
/** Called once after a durable handoff; an exception leaves the attempt unknown. */
export async function sendAttempt(token: string, attempt: Attempt): Promise<void> {
  if (attempt.instructions.escrow) {
    const { everSigningAvailable } = await import("./escrow");
    const current = await request<import("./escrow").Escrow>(`/trades/${attempt.instructions.trade_id}/escrow`, token);
    if (!everSigningAvailable(current)) throw new Error("USDT escrow is unavailable or the final 30-minute settlement window has started.");
  }
  const leg = attempt.instructions.leg;
  // The signed handoff carries its own network: the desk and peer settlement can sit on different chains.
  const net = evmNetworkFor(attempt.instructions.call.chain_id);
  const wallet = await connectWallet(walletNetwork(leg.chain, net));
  if (wallet.address.toLowerCase() !== leg.sender) throw new Error("Connected wallet differs from the booked sender. Reconcile this saved attempt before sending.");
  let reference: string;
  if (wallet.family === "evm") {
    if (wallet.chainId !== net.chainId) throw new Error(`Switch to ${net.label}.`);
    const { sendTransaction } = await import("@wagmi/core");
    const call = attempt.instructions.call;
    const nonce = BigInt(call.nonce!);
    if (nonce > BigInt(Number.MAX_SAFE_INTEGER)) throw new Error("Wallet nonce cannot be represented safely.");
    reference = await sendTransaction(wallet.config as Parameters<typeof sendTransaction>[0], { chainId: net.chainId, account: wallet.address, to: call.to as `0x${string}`, data: call.data as `0x${string}`, value: 0n, nonce: Number(nonce) });
  } else if (wallet.family === "everscale") {
    const state = await wallet.provider.request({ method: "getProviderState" });
    if (state.selectedConnection !== "mainnet") throw new Error("Switch EVER Wallet to Everscale mainnet.");
    if (attempt.instructions.escrow) {
      const { everSigningAvailable } = await import("./escrow");
      const current = await request<import("./escrow").Escrow>(`/trades/${attempt.instructions.trade_id}/escrow`, token);
      if (!everSigningAvailable(current)) throw new Error("USDT escrow is unavailable or the final 30-minute settlement window has started.");
    }
    const result = await wallet.provider.request({ method: "sendMessage", params: { sender: leg.sender, recipient: leg.recipient, amount: leg.transfer_amount, bounce: true } });
    reference = result.transaction.id.hash;
  } else throw new Error("Unsupported wallet.");
  // Keep the original result available if the reference POST response is lost.
  sessionStorage.setItem(`pay3flow.otc.reference.${attempt.id}`, reference);
  await request(attempt.instructions.submit_path || `/attempts/${attempt.id}/reference`, token, { reference });
}
