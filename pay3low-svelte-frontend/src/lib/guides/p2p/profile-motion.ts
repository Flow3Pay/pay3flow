import type { TutorialFrameKind } from "$lib/route-tutorial";

export const portion = (value: number, start: number, end: number) => Math.max(0, Math.min(1, (value - start) / (end - start)));
export const ease = (value: number) => value * value * (3 - 2 * value);
const horizontal = (percent: number, pixels = 0) => `calc(${percent}cqw + ${pixels}px)`;

/** Follow the shared player's clock, with continuous positions between scenes. */
export function profileMotion(kind: TutorialFrameKind | undefined, progress: number, actionY = 106) {
  const scroll = kind === "open" ? ease(portion(progress, .48, .9)) : 1;
  const approach = ease(portion(progress, 0, .45));
  const reviewTravel = ease(portion(progress, .35, .8));
  const adsApproach = ease(portion(progress, 0, .22));
  const tradeApproach = ease(portion(progress, .35, .85));
  return {
    scroll,
    reviews: kind === "review" ? progress >= .18 : kind === "verify" && progress < .22,
    cursorX: kind === "open" ? horizontal(87 * (1 - approach), 80 * approach)
      : kind === "review" ? horizontal(40 * reviewTravel, 80 * (1 - reviewTravel))
      : kind === "verify" ? horizontal(40 * (1 - adsApproach) + 88 * tradeApproach, 20 * (adsApproach - tradeApproach)) : "88cqw",
    cursorY: kind === "open" ? 30 + 120 * approach - 132 * scroll
      : kind === "review" ? 18 + 124 * reviewTravel
      : kind === "verify" ? 142 - 124 * adsApproach + (actionY - 18) * ease(portion(progress, .35, .7)) : actionY,
    tabClick: kind === "review" ? progress >= .18 && progress < .34 : kind === "verify" && progress >= .22 && progress < .38,
    clicked: kind === "act" && progress >= .5,
  };
}
