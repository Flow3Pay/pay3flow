import { encodeFunctionData, parseAbi } from "viem";
import type { Trade } from "./service";

export const USDT = "0xdac17f958d2ee523a2206206994597c13d831ec7";
export const MAX_EVER_UNITS = 100_000_000_000_000n;
export const ESCROW_SECONDS = 14_400;
export const EVER_CUTOFF_SECONDS = 1_800;
const vaultAbi = parseAbi(["function release()", "function refundEarly()", "function refundExpired()"]);

export interface EscrowPolicy {
  factory: string; factory_code_hash: string; arbitrator: string; singleton: string;
  singleton_code_hash: string;
}
export interface EscrowTypedData {
  domain: { chainId: number; verifyingContract: string; name?: string; version?: string };
  primaryType: string; types: Record<string, { name: string; type: string }[]>;
  message: Record<string, string | number>;
}
export interface EscrowCall {
  id: string; action: string; actor?: string; proposal_id?: string | null; state: string; reference: string | null;
  instructions: { chain_id: number; from: string; to: string; data: string; value: string; nonce: string; checkpoint: string };
}
export interface EscrowProposal {
  id: string; action: "release" | "refund_early"; hash: string; typed_data: EscrowTypedData;
  dispute: boolean; reason: string; evidence: Record<string, unknown>; approvals: { owner: string }[];
}
export interface Escrow {
  deployment: { trade_id: string; terms_hash: string; buyer: string; seller: string; usdt_amount: string; ever_amount: string; valid_until: number };
  policy: EscrowPolicy; typed_data: EscrowTypedData; safe: string | null; vault: string | null;
  created_at: string | null; expires_at: string | null; state: string; observed_at: string | null;
  consents: { owner: string }[]; proposals: EscrowProposal[]; calls: EscrowCall[];
  evidence: { kind: string; valid: boolean; evidence?: Record<string, unknown> }[];
}

function same(a: unknown, b: unknown): boolean { return typeof a === "string" && typeof b === "string" && a.toLowerCase() === b.toLowerCase(); }
export function validateEscrow(trade: Trade, escrow: Escrow): void {
  const policy = trade.terms.escrow;
  const buyer = trade.terms.direction === "buy" ? trade.terms.customer_usdt : trade.terms.desk_usdt;
  const seller = trade.terms.direction === "buy" ? trade.terms.desk_usdt : trade.terms.customer_usdt;
  const ever = BigInt(trade.terms.direction === "buy" ? trade.terms.output_units : trade.terms.input_units);
  const d = escrow.deployment;
  if (!policy || !same(policy.factory, escrow.policy.factory) || !same(policy.arbitrator, escrow.policy.arbitrator)
    || !same(d.buyer, buyer) || !same(d.seller, seller) || same(buyer, seller)
    || same(buyer, policy.arbitrator) || same(seller, policy.arbitrator)
    || ever <= 0n || ever > MAX_EVER_UNITS || BigInt(d.ever_amount) !== ever
    || BigInt(d.usdt_amount) !== BigInt(trade.terms.fee_policy.basis_units)
    || d.valid_until !== Math.floor(Date.parse(trade.terms.pay_by) / 1000)) throw new Error("Escrow differs from the accepted trade.");
  if (escrow.created_at && escrow.expires_at && Date.parse(escrow.expires_at) - Date.parse(escrow.created_at) !== ESCROW_SECONDS * 1000) throw new Error("Escrow lifetime must be four hours from creation.");
}

/** Validate the actual payload shown in a wallet, rather than only its API label. */
export function validateEscrowTypedData(trade: Trade, escrow: Escrow, data: EscrowTypedData, proposal?: EscrowProposal): void {
  validateEscrow(trade, escrow);
  const fields = proposal
    ? ["to:address", "value:uint256", "data:bytes", "operation:uint8", "safeTxGas:uint256", "baseGas:uint256", "gasPrice:uint256", "gasToken:address", "refundReceiver:address", "nonce:uint256"]
    : ["tradeId:bytes32", "termsHash:bytes32", "buyer:address", "seller:address", "arbitrator:address", "token:address", "usdtAmount:uint256", "everAmount:uint256", "validUntil:uint256"];
  const primary = proposal ? "SafeTx" : "EscrowConsent";
  if (Object.keys(data.types).length !== 1 || !Array.isArray(data.types[primary])
    || data.types[primary].map(field => `${field.name}:${field.type}`).join(",") !== fields.join(",")) throw new Error("Unexpected EIP-712 schema.");
  const message = data.message;
  if (data.domain.chainId !== 1) throw new Error("Escrow signatures must use Ethereum mainnet.");
  if (!proposal) {
    const d = escrow.deployment;
    if (data.primaryType !== "EscrowConsent" || !same(data.domain.verifyingContract, escrow.policy.factory)
      || data.domain.name !== "Pay3Flow OTC Escrow" || data.domain.version !== "1"
      || !same(message.tradeId, d.trade_id) || !same(message.termsHash, d.terms_hash)
      || !same(message.buyer, d.buyer) || !same(message.seller, d.seller)
      || !same(message.arbitrator, escrow.policy.arbitrator) || !same(message.token, USDT)
      || BigInt(message.usdtAmount) !== BigInt(d.usdt_amount) || BigInt(message.everAmount) !== BigInt(d.ever_amount)
      || BigInt(message.validUntil) !== BigInt(d.valid_until)) throw new Error("Deployment consent contains unexpected terms.");
  } else {
    const expected = encodeFunctionData({ abi: vaultAbi, functionName: proposal.action === "release" ? "release" : "refundEarly" });
    if (data.primaryType !== "SafeTx" || data.domain.name !== undefined || data.domain.version !== undefined || !same(data.domain.verifyingContract, escrow.safe)
      || !same(message.to, escrow.vault) || !same(message.data, expected) || BigInt(message.operation) !== 0n
      || ["value", "safeTxGas", "baseGas", "gasPrice"].some(key => BigInt(message[key]) !== 0n)
      || BigInt(message.gasToken) !== 0n || BigInt(message.refundReceiver) !== 0n) throw new Error("Safe proposal contains an unexpected transaction.");
  }
}

export function everSigningAvailable(escrow: Escrow, now = Date.now()): boolean {
  return escrow.state === "funded" && !!escrow.expires_at && !!escrow.observed_at
    && now - Date.parse(escrow.observed_at) < 60_000
    && Date.parse(escrow.expires_at) - now > EVER_CUTOFF_SECONDS * 1000
    && escrow.evidence.some(e => e.kind === "funding" && e.valid);
}
