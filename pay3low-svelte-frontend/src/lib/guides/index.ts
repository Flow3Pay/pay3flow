import type { ComponentType, SvelteComponent } from "svelte";
import type { RouteCandidate } from "$lib/exchange";
import type { TutorialStep } from "$lib/route-tutorial";
import WhitebirdSwapCard from "./whitebird/SwapCard.svelte";
import BybitProfileCard from "./bybit/ProfileCard.svelte";

export interface GuideSwapCardProps {
  route: RouteCandidate;
  step: TutorialStep;
  frame?: number;
  playing?: boolean;
}

// Each venue owns its swap illustration; the shared scene owns playback and captions.
export const guideSwapCards: Record<string, ComponentType<SvelteComponent<GuideSwapCardProps>> | undefined> = {
  whitebird: WhitebirdSwapCard,
};

export interface GuideP2pCardProps extends GuideSwapCardProps {
  progress?: number;
}

export const guideP2pCards: Record<string, ComponentType<SvelteComponent<GuideP2pCardProps>> | undefined> = {
  bybit: BybitProfileCard,
};
