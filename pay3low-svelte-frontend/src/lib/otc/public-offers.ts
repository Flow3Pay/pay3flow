import type { Direction } from "./service";

export type Stablecoin = "USDT" | "USDC";
export const offerStablecoin = (offer: Pick<PublicOffer, "terms">): Stablecoin =>
  offer.terms.stablecoin === "USDC" ? "USDC" : "USDT";

export interface PublicOffer {
  id: string;
  seller_ever: string;
  seller_avatar?: string;
  owner_ever?: string;
  state: string;
  created_at?: string;
  expires_at: string;
  terms: {
    direction?: Direction; stablecoin?: Stablecoin; ever_amount: string; usdt_amount: string; usdt_address: string;
    stablecoin_network?: string; stablecoin_asset_id?: string | null;
    ethereum_chain_id?: number; usdt_contract?: string; stablecoin_contract?: string;
  };
}

/** Sanitized immutable data for a public publication-history chart. */
export interface OfferHistoryPoint {
  id: string;
  created_at: string;
  terms: { direction: Direction; stablecoin?: Stablecoin; ever_amount: string; usdt_amount: string };
}

export { offerDecimal, offerTotal, offerEver, offerUnits, offerFloored, balanceUnits, offerShortfall, marketUsdtPerEver, marketUsdPerEver, everFromUsd, p2pFeeSplit, MAX_OFFER_EVER_UNITS, MAX_OFFER_USDT_UNITS, saleRate, compareOfferRates } from "./offer-amount";

export const offerDirection = (offer: PublicOffer): Direction => offer.terms.direction || "sell";
export const offerOwner = (offer: PublicOffer): string => offer.owner_ever || offer.seller_ever;

export interface SaleTicketState {
  direction: Direction;
  stablecoin: Stablecoin;
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
  stablecoinNetwork: string;
  supportedStablecoinAssets: Array<{ symbol: Stablecoin; network: string }>;
  seller: string;
  askingRate: string;
  valid: boolean;
  reviewing: boolean;
  linkingWallet: boolean;
  marketPrice: boolean;
  marketRate: string;
  /** USD per EVER from the same reference feed, used by the USD amount input. */
  marketUsdRate: string;
  /** Soft conversion hint for an empty stablecoin amount field. */
  usdtApprox: string;
  /** Soft conversion hint for an empty EVER amount field. */
  everApprox: string;
  error: string;
  onPrice: (value: string) => void;
  onEver: (value: string) => void;
  onUsdt: (value: string) => void;
  onStablecoin: (value: Stablecoin) => void;
  onAddress: (value: string) => void;
  onStablecoinNetwork: (value: string) => void;
  onUseWallet: () => void;
  onMarketPrice: (checked: boolean) => void;
  onRequest: () => void;
  onCloseReview: () => void;
}
