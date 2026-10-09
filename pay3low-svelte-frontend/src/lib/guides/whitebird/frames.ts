import type { TutorialFrame } from "$lib/route-tutorial";

export function buildWhitebirdFrames(copy: (key: string) => string): TutorialFrame[] {
  return [
    { kind: "open", title: copy("Open the exchange"), text: copy("Select the currencies you sell and receive on Whitebird.") },
    { kind: "verify", title: copy("Check the exchange"), text: copy("Check the amount you send, the selected network, the current rate and the service commission.") },
    { kind: "review", title: copy("Check the amount you receive"), text: copy("Review the quoted amount in the Receive field before exchanging.") },
    { kind: "act", title: copy("Press Exchange"), text: copy("Press Exchange on Whitebird, sign in or complete verification if requested, and follow the payment instructions shown there.") },
  ];
}
