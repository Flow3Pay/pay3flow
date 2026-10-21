import type { Direction } from "./service";

export interface PublicOffer {
  id: string;
  seller_ever: string;
  owner_ever?: string;
  state: string;
  created_at?: string;
  expires_at: string;
  terms: { direction?: Direction; ever_amount: string; usdt_amount: string; usdt_address: string };
}

/** Sanitized immutable data for a public publication-history chart. */
export interface OfferHistoryPoint {
  id: string;
  created_at: string;
  terms: { direction: Direction; ever_amount: string; usdt_amount: string };
}

export { offerDecimal, offerTotal, offerEver, validOfferAmount, saleRate, compareOfferRates } from "./offer-amount";

export const offerDirection = (offer: PublicOffer): Direction => offer.terms.direction || "sell";
export const offerOwner = (offer: PublicOffer): string => offer.owner_ever || offer.seller_ever;

export interface SaleTicketState {
  direction: Direction;
  price: string;
  priceRounded: boolean;
  roundingAsset: "USDT" | "EVER" | null;
  blockingReason: string;
  everAmount: string;
  usdtAmount: string;
  receivingAddress: string;
  seller: string;
  askingRate: string;
  valid: boolean;
  reviewing: boolean;
  linkingEvm: boolean;
  onPrice: (value: string) => void;
  onEver: (value: string) => void;
  onUsdt: (value: string) => void;
  onAddress: (value: string) => void;
  onUseEthereum: () => void;
  onRequest: () => void;
}
