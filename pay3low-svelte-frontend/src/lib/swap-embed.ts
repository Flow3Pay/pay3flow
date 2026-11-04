import type { RouteCandidate, RouteExecutionDescriptor } from "./exchange";
import type { TutorialStep } from "./route-tutorial";

const EXECUTABLE_PROVIDERS = ["cow-swap", "symbiosis", "near-intents"];

/**
 * A swap widget is shown only when the backend issued an execution token for
 * this exact leg. Exchange guides (MEXC, Bybit, banks, …) and legs without a
 * Trustfile-backed execution path stay guides.
 */
export function executionFor(
  step: TutorialStep | undefined,
  route: RouteCandidate,
): RouteExecutionDescriptor | undefined {
  if (!step || step.kind !== "swap") return undefined;
  const provider = step.provider.toLowerCase();
  const descriptors = route.executions?.length
    ? route.executions
    : route.execution
      ? [route.execution]
      : [];
  return descriptors.find(
    (item) =>
      item.provider.toLowerCase() === provider &&
      (!step.fromAsset || item.from_asset === step.fromAsset),
  );
}

export function canOpenSwap(step: TutorialStep | undefined, route: RouteCandidate): boolean {
  if (step?.kind !== "swap") return false;
  const provider = step.provider.toLowerCase();
  if (provider === "pay3flow-otc") return true;
  return step.execution === true && EXECUTABLE_PROVIDERS.includes(provider) && Boolean(executionFor(step, route));
}
