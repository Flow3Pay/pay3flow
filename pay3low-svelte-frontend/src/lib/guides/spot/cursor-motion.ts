import type { TutorialFrameKind } from "$lib/route-tutorial";
import { ease, portion } from "../p2p/profile-motion";

type CursorState = { kind: TutorialFrameKind | undefined; progress: number };
const targets: Record<TutorialFrameKind, string> = {
  open: ".pair",
  verify: ".sideTabs .selected",
  review: ".amountField",
  act: ".tradeAction",
  receive: ".historyTabs > span:nth-child(2)",
};
const preceding: Record<TutorialFrameKind, TutorialFrameKind> = { open: "open", verify: "open", review: "verify", act: "review", receive: "act" };

/** Resolve targets when the layout changes, then animate on the player's shared clock. */
export function cursorMotion(node: SVGSVGElement, initial: CursorState) {
  const screen = node.closest<HTMLElement>(".screen");
  if (!screen) return;
  let state = initial;
  let start = { x: 0, y: 0 }, end = start;
  function locate(kind: TutorialFrameKind) {
    const target = screen!.querySelector(targets[kind]);
    if (!target) return { x: 0, y: 0 };
    const box = target.getBoundingClientRect(), parent = screen!.getBoundingClientRect();
    return { x: box.left - parent.left + box.width * .72, y: box.top - parent.top + box.height * .7 };
  }
  function draw() {
    const travel = ease(portion(state.progress, 0, .72));
    node.style.left = `${start.x + (end.x - start.x) * travel}px`;
    node.style.top = `${start.y + (end.y - start.y) * travel}px`;
  }
  function measure() {
    const kind = state.kind ?? "open";
    start = locate(preceding[kind]); end = locate(kind);
    if (kind === "open") start = { x: end.x + 55, y: end.y + 20 };
    draw();
  }
  const observer = new ResizeObserver(measure);
  observer.observe(screen);
  measure();
  return {
    update(next: CursorState) { const changed = next.kind !== state.kind; state = next; if (changed) measure(); else draw(); },
    destroy() { observer.disconnect(); },
  };
}
