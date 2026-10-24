import safeAbi from "./everscale-safe.abi.json" with { type: "json" };
export const SAFE_ABI = JSON.stringify(safeAbi);
/** Public wire format shared with evernode-rs otc export. Never accepts keys or seeds. */
export interface EverParticipant { public_key: string; address: string }
export interface EverAgreement {
  schema_version: 1;
  terms: { trade_id: string; network_id: 42; buyer: EverParticipant; seller: EverParticipant; arbitrator: string; deployment_key: string; amount: string };
  wallet_address: string;
}
export interface EverOperation { public_key: string; action: "release" | "refund"; transaction_id: string | null }
export interface EverCall { publicKey: string; recipient: string; payload: { abi: string; method: string; params: Record<string, unknown> } }
export interface EverReview {
  trade: EverAgreement; terms_hash: string; verified: boolean; balance_nanotokens: string;
  owners: { index: number; public_key: string }[];
  transactions: { id: string; action: "release" | "refund" | null; destination: string; amount: string; confirmations_mask: string }[];
  call: EverCall | null; handoff?: { id: string; terms_hash: string } | null;
}
export interface EverHandoff {
  id: string; state: "unknown" | "accepted" | "failed"; reference: string | null; resumed: boolean;
  instructions: { request: { trade: EverAgreement; terms_hash: string; operation: EverOperation }; review: EverReview };
}
export interface EverPending { id: string; terms_hash: string; reference: string | null; prepare_key?: string; phase?: "preparing" | "signing"; request?: { trade: EverAgreement; terms_hash: string; operation: EverOperation } }

function record(value: unknown, keys: string[]): Record<string, any> {
  if (!value || typeof value !== "object" || Array.isArray(value) || Object.keys(value).length !== keys.length || keys.some(key => !Object.hasOwn(value, key))) throw new Error("Use only the public trade.json exported by evernode-rs. Private key and seed files are not accepted.");
  return value as Record<string, any>;
}
function address(value: unknown): string {
  if (typeof value !== "string" || !/^(0|-1):[0-9a-f]{64}$/.test(value) || /^-?\d+:0{64}$/.test(value)) throw new Error("Invalid Everscale address in agreement.");
  return value;
}
function publicKey(value: unknown): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value) || /^0{64}$/.test(value)) throw new Error("Invalid public key in agreement.");
  return value;
}
function participant(value: unknown): EverParticipant {
  const item = record(value, ["public_key", "address"]);
  return { public_key: publicKey(item.public_key), address: address(item.address) };
}
export function parseAgreement(text: string): EverAgreement {
  if (text.length > 16384) throw new Error("Agreement is too large.");
  const raw = record(JSON.parse(text), ["schema_version", "terms", "wallet_address"]);
  const terms = record(raw.terms, ["trade_id", "network_id", "buyer", "seller", "arbitrator", "deployment_key", "amount"]);
  if (raw.schema_version !== 1 || terms.network_id !== 42 || typeof terms.trade_id !== "string" || !/^[A-Za-z0-9][A-Za-z0-9_-]{0,63}$/.test(terms.trade_id)) throw new Error("Expected a version 1 Everscale mainnet agreement.");
  if (typeof terms.amount !== "string" || !/^[1-9][0-9]*$/.test(terms.amount) || BigInt(terms.amount) < 1_000_000n || BigInt(terms.amount) >= 2n ** 128n) throw new Error("Amount must be an exact uint128 nanotoken string of at least 0.001 EVER.");
  const buyer = participant(terms.buyer), seller = participant(terms.seller), arbitrator = publicKey(terms.arbitrator);
  const wallet = address(raw.wallet_address);
  if (new Set([buyer.public_key, seller.public_key, arbitrator]).size !== 3 || buyer.address === seller.address || [buyer.address,seller.address].includes(wallet)) throw new Error("Agreement requires three distinct owners and separate release, refund and multisig addresses.");
  // Rust serde serializes in this exact declaration order for the agreement hash.
  return { schema_version: 1, terms: { trade_id: terms.trade_id, network_id: 42, buyer, seller, arbitrator, deployment_key: publicKey(terms.deployment_key), amount: terms.amount }, wallet_address: wallet };
}
export async function agreementHash(trade: EverAgreement): Promise<string> {
  const bytes = new TextEncoder().encode(`evernode-otc/v1\0${JSON.stringify(trade)}`);
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return Array.from(new Uint8Array(digest), b => b.toString(16).padStart(2, "0")).join("");
}
export function validateCall(trade: EverAgreement, operation: EverOperation, input: unknown): asserts input is EverCall {
  const call = record(input, ["publicKey", "recipient", "payload"]);
  const payload = record(call.payload, ["abi", "method", "params"]);
  if (call.publicKey !== operation.public_key || ![trade.terms.buyer.public_key,trade.terms.seller.public_key,trade.terms.arbitrator].includes(call.publicKey) || call.recipient !== trade.wallet_address || typeof payload.abi !== "string") throw new Error("Wallet call differs from the reviewed agreement.");
  if (JSON.stringify(JSON.parse(payload.abi)) !== SAFE_ABI) throw new Error("Wallet ABI differs from the pinned SafeMultisig contract.");
  if (!["release","refund"].includes(operation.action)) throw new Error("Invalid OTC action.");
  if (operation.transaction_id !== null) {
    const params = record(payload.params, ["transactionId"]);
    if (!/^[1-9][0-9]*$/.test(operation.transaction_id) || BigInt(operation.transaction_id) >= 2n ** 64n || payload.method !== "confirmTransaction" || params.transactionId !== operation.transaction_id) throw new Error("Confirmation differs from the reviewed transaction ID.");
  } else {
    const params = record(payload.params, ["dest","value","bounce","allBalance","payload"]);
    if (payload.method !== "submitTransaction" || params.dest !== trade.terms[operation.action === "release" ? "buyer" : "seller"].address || params.value !== trade.terms.amount || params.bounce !== true || params.allBalance !== false || params.payload !== "") throw new Error("Transfer differs from the reviewed amount or destination.");
  }
}
export function pendingStorageKey(actor: string): string { return `pay3flow.otc.ever-multisig.pending.v1:${actor}`; }
