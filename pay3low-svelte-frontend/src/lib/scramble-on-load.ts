import { get, writable } from "svelte/store";

const GLYPHS = "#%&*+=-_/\\<>[]{}?!$@0123456789";
const STEP_MS = 22;
const JITTER_MS = 60;
const RESOLVE_MS = 120;
const RESOLVE_JITTER_MS = 200;
const GLYPH_REFRESH_MS = 50;
const NODE_STAGGER_MS = 6;
const MAX_NODE_DELAY_MS = 420;
const NODE_SPAN_MS = 480;
const MIN_LETTERS = 3;

/** Live quotes, amounts and copy with its own decoder stay out of this pass. */
export const SKIP_SELECTORS = [
  "[data-no-scramble]", ".srOnly", ".scrambleBlock", ".amountInput", ".marketValue", ".routeAmount",
  ".rateRow", "table", "noscript", "style", "script", "template", "select", "textarea",
].join(",");

/** Long-form pages and guides are static on purpose. */
const SKIP_PATHS = [/^\/about(?:\/|$)/, /^\/terms(?:\/|$)/, /^\/otc-runbook(?:\/|$)/, /^\/share\/guide(?:\/|$)/];
const SKIP_HASH = /^#\/guide(?:[/?]|$)/;

const LETTER = /\p{L}/u;

/** Set while a text node carries temporary glyphs, so `localize` leaves it alone. */
export const scramblingNodes = new WeakSet<Text>();

/** The load pass waits for the startup animation on the product page. */
export const introFinished = writable(true);

export function countLetters(value: string) {
  let total = 0;
  for (const char of value) if (LETTER.test(char)) total++;
  return total;
}

export function isScrambableText(value: string) {
  return countLetters(value) >= MIN_LETTERS;
}

export function isSkippedPath(pathname: string, hash = "") {
  return SKIP_PATHS.some((pattern) => pattern.test(pathname)) || SKIP_HASH.test(hash);
}

export type NodeRun = {
  text: Text;
  chars: string[];
  letters: boolean[];
  starts: number[];
  ends: number[];
  glyphs: string[];
  glyphAt: number[];
  maxEnd: number;
  written: string;
};

export function planRun(value: string, delay: number, random: () => number = Math.random): NodeRun | null {
  if (!isScrambableText(value)) return null;
  const chars = Array.from(value);
  // Long paragraphs would take seconds at the component step, so they decode faster.
  const step = chars.length > 1 ? Math.min(STEP_MS, NODE_SPAN_MS / chars.length) : 0;
  const letters = chars.map((char) => LETTER.test(char));
  const starts = new Array<number>(chars.length).fill(0);
  const ends = new Array<number>(chars.length).fill(0);
  let maxEnd = 0;
  for (let i = 0; i < chars.length; i++) {
    if (!letters[i]) continue;
    const start = delay + i * step + random() * JITTER_MS;
    const end = start + RESOLVE_MS + random() * RESOLVE_JITTER_MS;
    starts[i] = start;
    ends[i] = end;
    if (end > maxEnd) maxEnd = end;
  }
  if (!maxEnd) return null;
  return {
    text: null as unknown as Text,
    chars,
    letters,
    starts,
    ends,
    glyphs: new Array<string>(chars.length).fill(GLYPHS[0]),
    glyphAt: new Array<number>(chars.length).fill(-GLYPH_REFRESH_MS),
    maxEnd,
    written: value,
  };
}

export function frameText(run: NodeRun, elapsed: number, now: number, random: () => number = Math.random) {
  let out = "";
  for (let i = 0; i < run.chars.length; i++) {
    const char = run.chars[i];
    if (!run.letters[i] || elapsed < run.starts[i] || elapsed >= run.ends[i]) {
      out += char;
      continue;
    }
    if (now - run.glyphAt[i] >= GLYPH_REFRESH_MS) {
      run.glyphs[i] = GLYPHS[Math.floor(random() * GLYPHS.length)];
      run.glyphAt[i] = now;
    }
    out += run.glyphs[i];
  }
  return out;
}

function collectRuns(root: ParentNode): NodeRun[] {
  const runs: NodeRun[] = [];
  const viewport = window.visualViewport?.height ?? window.innerHeight;
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
    acceptNode(node) {
      const parent = node.parentElement;
      if (!parent || parent.closest(SKIP_SELECTORS)) return NodeFilter.FILTER_REJECT;
      return NodeFilter.FILTER_ACCEPT;
    },
  });
  let index = 0;
  let current: Node | null;
  while ((current = walker.nextNode())) {
    const text = current as Text;
    const value = text.nodeValue ?? "";
    if (!isScrambableText(value)) continue;
    const box = text.parentElement!.getBoundingClientRect();
    if (box.width <= 0 || box.height <= 0) continue;
    if (box.bottom < 0 || box.top > viewport) continue;
    const run = planRun(value, Math.min(index * NODE_STAGGER_MS, MAX_NODE_DELAY_MS));
    if (!run) continue;
    run.text = text;
    scramblingNodes.add(text);
    runs.push(run);
    index++;
  }
  return runs;
}

function finishRun(run: NodeRun) {
  scramblingNodes.delete(run.text);
  if (run.text.isConnected && run.text.nodeValue === run.written) run.text.nodeValue = run.chars.join("");
}

/**
 * Decodes the copy that is on screen once, after the startup animation finishes.
 * Numbers and currency codes inside amount readouts are never touched.
 */
export function scrambleOnLoad(node: HTMLElement) {
  let started = false;
  let raf = 0;
  let runs: NodeRun[] = [];
  let startedAt = 0;

  function cancel() {
    if (raf) window.cancelAnimationFrame(raf);
    raf = 0;
    for (const run of runs) finishRun(run);
    runs = [];
  }

  function step(now: number) {
    raf = 0;
    if (!runs.length) return;
    const elapsed = now - startedAt;
    for (let i = runs.length - 1; i >= 0; i--) {
      const run = runs[i];
      if (!run.text.isConnected) {
        scramblingNodes.delete(run.text);
        runs.splice(i, 1);
        continue;
      }
      // The copy was replaced while decoding, so leave whatever is there now.
      if (run.text.nodeValue !== run.written) {
        scramblingNodes.delete(run.text);
        runs.splice(i, 1);
        continue;
      }
      if (elapsed >= run.maxEnd) {
        finishRun(run);
        runs.splice(i, 1);
        continue;
      }
      const next = frameText(run, elapsed, now);
      if (next !== run.written) {
        run.text.nodeValue = next;
        run.written = next;
      }
    }
    if (runs.length) raf = window.requestAnimationFrame(step);
  }

  function begin() {
    if (started) return;
    if (!get(introFinished)) return;
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    if (isSkippedPath(window.location.pathname, window.location.hash)) return;
    started = true;
    runs = collectRuns(node);
    if (!runs.length) return;
    startedAt = performance.now();
    raf = window.requestAnimationFrame(step);
  }

  const unsubscribe = introFinished.subscribe((ready) => {
    if (ready && !started) window.requestAnimationFrame(begin);
  });

  return { destroy: cancel };
}
