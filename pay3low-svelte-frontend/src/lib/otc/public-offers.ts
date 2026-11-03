import type { Direction } from "./service";

export interface PublicOffer {
  id: string;
  seller_ever: string;
  owner_ever?: string;
  state: string;
  created_at?: string;
  expires_at: string;
  terms: {
    direction?: Direction; ever_amount: string; usdt_amount: string; usdt_address: string;
    ethereum_chain_id?: number; usdt_contract?: string;
  };
}

/** Sanitized immutable data for a public publication-history chart. */
export interface OfferHistoryPoint {
  id: string;
  created_at: string;
  terms: { direction: Direction; ever_amount: string; usdt_amount: string };
}

export { offerDecimal, offerTotal, offerEver, offerUnits, offerFloored, balanceUnits, offerShortfall, marketUsdtPerEver, p2pFeeSplit, MAX_OFFER_EVER_UNITS, MAX_OFFER_USDT_UNITS, saleRate, compareOfferRates } from "./offer-amount";

export const offerDirection = (offer: PublicOffer): Direction => offer.terms.direction || "sell";
export const offerOwner = (offer: PublicOffer): string => offer.owner_ever || offer.seller_ever;

export interface SaleTicketState {
  direction: Direction;
  price: string;
  priceRounded: boolean;
  roundingAsset: "USDT" | "EVER" | null;
  amountFloored: "USDT" | "EVER" | null;
  /** The leg the trader pays from their own wallet, when the entered amount is above what it holds. */
  shortfall: "ever" | "usdt" | null;
  /** Wallet balance behind {@link shortfall}, in the asset's own decimals; empty when no balance was read. */
  fundedBalance: string;
  blockingReason: string;
  everAmount: string;
  usdtAmount: string;
  receivingAddress: string;
  seller: string;
  askingRate: string;
  valid: boolean;
  reviewing: boolean;
  linkingEvm: boolean;
  marketPrice: boolean;
  marketRate: string;
  marketApprox: string;
  error: string;
  onPrice: (value: string) => void;
  onEver: (value: string) => void;
  onUsdt: (value: string) => void;
  onAddress: (value: string) => void;
  onUseEthereum: () => void;
  onMarketPrice: (checked: boolean) => void;
  onRequest: () => void;
  onCloseReview: () => void;
}
