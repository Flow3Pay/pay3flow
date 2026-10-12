import type { ComponentType, SvelteComponent } from "svelte";
import type { RouteCandidate } from "$lib/exchange";
import type { TutorialStep } from "$lib/route-tutorial";
import WhitebirdSwapCard from "./whitebird/SwapCard.svelte";
import BybitProfileCard from "./bybit/ProfileCard.svelte";
import MexcProfileCard from "./mexc/ProfileCard.svelte";
import BinanceProfileCard from "./binance/ProfileCard.svelte";
import BestchangeExchangerCard from "./bestchange/ExchangerCard.svelte";
import VenueSwapCard from "./exchangers/SwapCard.svelte";
import WalletSwapCard from "./exchangers/WalletSwapCard.svelte";
import SpotCard from "./spot/SpotCard.svelte";
import CifraTradernetCard from "./cifra/TradernetCard.svelte";

export interface GuideSwapCardProps {
  route: RouteCandidate;
  step: TutorialStep;
  frame?: number;
  playing?: boolean;
  progress?: number;
}

// Each venue owns its swap illustration; the shared scene owns playback and captions.
export const guideSwapCards: Record<string, ComponentType<SvelteComponent<GuideSwapCardProps>> | undefined> = {
  whitebird: WhitebirdSwapCard,
  bestchange: BestchangeExchangerCard,
  bncex: VenueSwapCard,
  "bitcoin-center": VenueSwapCard,
  dzengi: VenueSwapCard,
  symbiosis: WalletSwapCard,
  "cow-swap": WalletSwapCard,
  "cifra-broker": CifraTradernetCard,
};

export type GuideP2pCardProps = GuideSwapCardProps;

export const guideSpotCards: Record<string, ComponentType<SvelteComponent<GuideSwapCardProps>> | undefined> = {
  binance: SpotCard,
  bybit: SpotCard,
  mexc: SpotCard,
  bitget: SpotCard,
  whitebird: SpotCard,
};

export const guideP2pCards: Record<string, ComponentType<SvelteComponent<GuideP2pCardProps>> | undefined> = {
  bybit: BybitProfileCard,
  mexc: MexcProfileCard,
  binance: BinanceProfileCard,
};
