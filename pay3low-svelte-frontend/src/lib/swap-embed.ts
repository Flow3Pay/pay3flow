import type { RouteCandidate } from "./exchange";
import type { TutorialStep } from "./route-tutorial";

/**
 * A swap widget is shown only when the backend says this exact route has an
 * executable provider swap. Exchange guides (MEXC, Bybit, banks, …) and
 * providers without a Trustfile-backed execution path stay guides.
 */
export function canOpenSwap(step: TutorialStep | undefined, route: RouteCandidate): boolean {
  if (step?.kind !== "swap") return false;
  const provider = step.provider.toLowerCase();
  if (provider === "pay3flow-otc") return true;
  return Boolean(
    step.execution &&
      route.execution &&
      route.execution.provider.toLowerCase() === provider,
  );
}
