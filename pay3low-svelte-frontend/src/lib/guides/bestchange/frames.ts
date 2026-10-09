import type { TutorialFrame, TutorialStep } from "$lib/route-tutorial";
import type { RouteCandidate } from "$lib/exchange";

type Copy = (key: string, params?: Record<string, string | number>) => string;

export function bestchangeExchanger(route: RouteCandidate, step: TutorialStep, copy: Copy): string {
  return step.offer?.advertiser.nickname
    ?? route.cycle_legs?.find((leg, index) => step.id === `cycle-${index}`)?.description
    ?? route.warnings?.find(warning => warning.startsWith("Quoted exchanger: "))?.replace(/^Quoted exchanger: |\.$/g, "")
    ?? copy("Selected exchanger");
}

export function buildBestchangeFrames(from: string, to: string, copy: Copy): TutorialFrame[] {
  return [
    { kind: "open", title: copy("Open the exchanger profile"), text: copy("Open the selected exchanger's BestChange page and check its name.") },
    { kind: "review", title: copy("Read the reviews"), text: copy("Scroll down to Reviews. Read recent feedback and any complaints, including their replies.") },
    { kind: "verify", title: copy("Return to the exchanger"), text: copy("Scroll back up and check the {from} → {to} exchange direction before leaving BestChange.", { from, to }) },
    { kind: "act", title: copy("Go to the exchanger website"), text: copy("Press the link to the selected exchanger's website. This walkthrough ends at that transition.") },
  ];
}
