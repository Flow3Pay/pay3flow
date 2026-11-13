import type { Attempt } from "./service";

export type P2pLeg = "payment" | "payout";
const directNetworks = new Set([
  "ethereum", "optimism", "bnb-smart-chain", "gnosis",
  "polygon-pos", "avalanche-c", "arbitrum-one", "base", "sei",
  "tron", "ton", "solana",
]);
const evmDirectNetworks = new Set([
  "ethereum", "optimism", "bnb-smart-chain", "gnosis",
  "polygon-pos", "avalanche-c", "arbitrum-one", "base", "sei",
]);

/**
 * A sell offer books the taker's Ethereum wallet as `usdt_payer`; that address
 * is only a valid sender on EVM networks. On a non-EVM receiving network the
 * taker must use the routed payment flow instead of a direct transfer.
 */
export function p2pDirectDirectionAllowed(direction: "buy" | "sell", network: string): boolean {
  return directNetworks.has(network)
    && (direction === "buy" || evmDirectNetworks.has(network));
}

export function p2pCompatibleSources<T extends { destination_asset_ids: string[] }>(
  sources: T[],
  destinationAssetId: string,
): T[] {
  return destinationAssetId
    ? sources.filter(source => source.destination_asset_ids.includes(destinationAssetId))
    : [];
}

export function p2pDirectAssetMatch(
  source: { asset: string; asset_id: string } | null,
  destinationAsset: string,
  destinationAssetId: string,
  destinationNetwork: string,
): boolean {
  return !!source
    && source.asset === destinationAsset
    && source.asset_id === destinationAssetId
    && directNetworks.has(destinationNetwork)
    && source.asset.split("@")[1] === destinationNetwork;
}

export interface P2pFeePolicy {
  version: string;
  rate: string;
  floor: string;
  basis_units: string;
  fee_units: string;
  net_units: string;
  precision: number;
}

export interface P2pDealTerms {
  direction: "buy" | "sell";
  stablecoin?: "USDT" | "USDC";
  stablecoin_network?: string;
  stablecoin_asset_id?: string | null;
  ever_amount: string;
  usdt_amount: string;
  ever_units: string;
  usdt_units: string;
  usdt_payer: string;
  usdt_payee: string;
  /** Network and exact asset identity the stablecoin payer sends from. */
  payer_network?: string;
  payer_asset_id?: string | null;
  ever_payer: string;
  ever_payee: string;
  fee_payer_actor?: string;
  fee_usdt_asset_id?: string | null;
  fee_policy?: P2pFeePolicy;
}

export interface P2pDeal {
  id: string;
  offer_id: string;
  state: string;
  role: "owner" | "taker";
  counterparty: string;
  terms: P2pDealTerms;
  fee_policy?: P2pFeePolicy;
  fee_accrued?: boolean;
  pay_by: string;
  payout_by: string | null;
  /** End of the 24-hour deal lifetime; the backend closes open deals past it. */
  expires_at?: string;
  created_at: string;
  attempts?: Attempt[];
  my_leg?: P2pLeg;
  counterparty_leg?: P2pLeg;
  my_ever?: string;
  counterparty_ever?: string;
  my_avatar?: string;
  counterparty_avatar?: string;
  my_confirmed?: boolean;
  counterparty_confirmed?: boolean;
  owner_confirmed?: boolean;
  taker_confirmed?: boolean;
}

export function p2pActive(deal: P2pDeal): boolean {
  return deal.state === "payment_pending" || deal.state === "delivery_pending";
}

export function p2pBothConfirmed(deal: P2pDeal): boolean {
  return !!deal.my_confirmed && !!deal.counterparty_confirmed;
}

/** The leg the current viewer has to sign in the current deal state. */
export function p2pKind(deal: P2pDeal): P2pLeg | null {
  if (deal.state === "payment_pending" && deal.my_leg === "payment") return "payment";
  if (deal.state === "delivery_pending" && deal.my_leg === "payout") return "payout";
  return null;
}

export function p2pOpenAttempt(deal: P2pDeal, kind: P2pLeg): Attempt | null {
  const attempts = deal.attempts || [];
  for (let index = attempts.length - 1; index >= 0; index -= 1) {
    const attempt = attempts[index];
    if (attempt.kind === kind && attempt.state !== "failed" && attempt.state !== "verified") return attempt;
  }
  return null;
}

export function p2pFailedAttempt(deal: P2pDeal, kind: P2pLeg): Attempt | null {
  const attempts = deal.attempts || [];
  for (let index = attempts.length - 1; index >= 0; index -= 1) {
    const attempt = attempts[index];
    if (attempt.kind === kind && attempt.state === "failed") return attempt;
  }
  return null;
}

/** What the single primary button would do right now. */
export function p2pAction(deal: P2pDeal): "confirm" | "send" | "retry" | null {
  if (!p2pActive(deal)) return null;
  if (!deal.my_confirmed) return "confirm";
  if (!p2pBothConfirmed(deal)) return null;
  const kind = p2pKind(deal);
  if (!kind) return null;
  if (kind === "payout" && p2pFeeStatus(deal) !== "collected") return null;
  const open = p2pOpenAttempt(deal, kind);
  if (open) return open.state === "prepared" ? "send" : null;
  if (p2pFailedAttempt(deal, kind)) return "retry";
  return "send";
}

export function p2pLegDone(deal: P2pDeal, kind: P2pLeg): boolean {
  if (kind === "payment") {
    return deal.state === "delivery_pending" || deal.state === "completed"
      || (deal.attempts || []).some(attempt => attempt.kind === kind && attempt.state === "verified");
  }
  return deal.state === "completed"
    || (deal.attempts || []).some(attempt => attempt.kind === kind && attempt.state === "verified");
}

/** Token transferred by a deal leg. */
export function p2pLegAsset(kind: P2pLeg, stablecoin: "USDT" | "USDC" = "USDT"): "USDT" | "USDC" | "EVER" {
  return kind === "payment" ? stablecoin : "EVER";
}

export interface P2pLegAmount {
  asset: "USDT" | "USDC" | "EVER";
  amount: string;
}

export interface P2pViewerLegs {
  send: P2pLegAmount;
  receive: P2pLegAmount;
}

/** The exact token amounts this viewer sends and receives in the deal. */
export function p2pViewerLegs(deal: P2pDeal): P2pViewerLegs | null {
  if (!deal.my_leg) return null;
  const netAmount = deal.fee_policy
    ? p2pFormatUnits(deal.fee_policy.net_units, deal.fee_policy.precision)
    : deal.terms.usdt_amount;
  const send = deal.my_leg === "payment"
    ? { asset: p2pLegAsset("payment", deal.terms.stablecoin), amount: netAmount }
    : { asset: p2pLegAsset("payout"), amount: deal.terms.ever_amount };
  const receive = deal.my_leg === "payment"
    ? { asset: p2pLegAsset("payout"), amount: deal.terms.ever_amount }
    : { asset: p2pLegAsset("payment", deal.terms.stablecoin), amount: netAmount };
  return { send, receive };
}

/** True once the nominal payment or delivery window has passed. */
export function p2pOverdue(deal: P2pDeal, now = Date.now()): boolean {
  if (deal.state === "payment_pending") return Date.parse(deal.pay_by) < now;
  if (deal.state === "delivery_pending") {
    return !!deal.payout_by && Date.parse(deal.payout_by) < now;
  }
  return false;
}

export function p2pLegActive(deal: P2pDeal, kind: P2pLeg): boolean {
  return p2pBothConfirmed(deal) && p2pKind(deal) === kind && !p2pLegDone(deal, kind);
}

export function p2pLegFailed(deal: P2pDeal, kind: P2pLeg): boolean {
  const attempt = p2pFailedAttempt(deal, kind);
  return !!attempt && !p2pOpenAttempt(deal, kind);
}

export function p2pShortAddress(value: string | undefined, head = 6, tail = 4): string {
  if (!value) return "";
  if (value.length <= head + tail + 1) return value;
  return `${value.slice(0, head)}…${value.slice(-tail)}`;
}

export function p2pFeeStatus(deal: P2pDeal): "not_due" | "accrued" | "pending" | "collected" {
  if (!deal.fee_accrued) return "not_due";
  const feeAttempts = (deal.attempts || []).filter(attempt => attempt.kind === "fee");
  if (feeAttempts.some(attempt => attempt.state === "verified")) return "collected";
  if (feeAttempts.some(attempt => attempt.state !== "failed")) return "pending";
  return "accrued";
}

export function p2pFormatUnits(units: string, precision = 6): string {
  if (!/^\d+$/.test(units) || !Number.isInteger(precision) || precision < 0) return "";
  const digits = units.padStart(precision + 1, "0");
  if (precision === 0) return digits;
  const whole = digits.slice(0, -precision);
  const fraction = digits.slice(-precision).replace(/0+$/, "");
  return fraction ? `${whole}.${fraction}` : whole;
}
