import { fallbackVenue } from "./provider-profile";
import type { RouteCandidate } from "./exchange";
import type { TutorialStep } from "./route-tutorial";

/** A swap action must open a wallet swap provider, rather than a bank or spot guide. */
export function canOpenSwap(step: TutorialStep | undefined, route: RouteCandidate): boolean {
  return Boolean(step?.kind === "swap" && (
    fallbackVenue(step.provider.toLowerCase()).types.includes("swap") ||
    (step.execution && route.execution)
  ));
}
