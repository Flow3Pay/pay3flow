<script lang="ts">
  import { onMount } from "svelte";

  export let phrases: string[][] = [["Move money.", "Keep more."]];
  export let active = true;
  export let holdMs = 5000;

  const GLYPHS = "#%&*+=-_/\\<>[]{}?!$@0123456789";
  const STEP_MS = 28;
  const JITTER_MS = 120;
  const RESOLVE_MS = 180;
  const RESOLVE_JITTER_MS = 420;
  const GLYPH_REFRESH_MS = 50;

  type CharLines = [string[], string[]];
  type Run = {
    from: CharLines;
    to: CharLines;
    positions: [number, number];
    offsets: [number, number];
    starts: number[];
    ends: number[];
    glyphs: string[];
    glyphAt: number[];
    maxEnd: number;
    startedAt: number;
  };

  let lineOne: HTMLSpanElement;
  let lineTwo: HTMLSpanElement;
  let mounted = false;
  let reduced = false;
  let phraseIndex = 0;
  let currentLines: [string, string] = ["", ""];

  let run: Run | null = null;
  let raf = 0;
  let holdTimer = 0;
  let holdEndsAt = 0;
  let holdRemaining = 0;
  let pausedHold = false;
  let pausedAt = 0;
  let lastPhrases = phrases;
  let lastActive = active;

  $: phraseCount = Math.max(1, phrases.length);
  $: firstSentence = (phrases[0] ?? []).join(" ");

  function linesOf(phrase: string[] | undefined): [string, string] {
    return [phrase?.[0] ?? "", phrase?.[1] ?? ""];
  }

  function setStatic(index: number) {
    const lines = linesOf(phrases[index]);
    phraseIndex = index;
    currentLines = lines;
    if (lineOne) lineOne.textContent = lines[0];
    if (lineTwo) lineTwo.textContent = lines[1];
  }

  function stopCycle() {
    if (raf) window.cancelAnimationFrame(raf);
    if (holdTimer) window.clearTimeout(holdTimer);
    raf = 0;
    holdTimer = 0;
    run = null;
    holdEndsAt = 0;
    holdRemaining = 0;
    pausedHold = false;
  }

  function buildRun(from: [string, string], to: [string, string]): Run {
    const fromChars: CharLines = [Array.from(from[0]), Array.from(from[1])];
    const toChars: CharLines = [Array.from(to[0]), Array.from(to[1])];
    const positions: [number, number] = [
      Math.max(fromChars[0].length, toChars[0].length),
      Math.max(fromChars[1].length, toChars[1].length),
    ];
    const offsets: [number, number] = [0, positions[0]];
    const length = positions[0] + positions[1];
    const starts = new Array<number>(length);
    const ends = new Array<number>(length);
    let maxEnd = 0;

    for (let i = 0; i < length; i++) {
      const start = i * STEP_MS + Math.random() * JITTER_MS;
      const end = start + RESOLVE_MS + Math.random() * RESOLVE_JITTER_MS;
      starts[i] = start;
      ends[i] = end;
      if (end > maxEnd) maxEnd = end;
    }

    return {
      from: fromChars,
      to: toChars,
      positions,
      offsets,
      starts,
      ends,
      glyphs: new Array<string>(length).fill(GLYPHS[0]),
      glyphAt: new Array<number>(length).fill(0),
      maxEnd,
      startedAt: 0,
    };
  }

  function render(runAt: Run, elapsed: number, now: number) {
    for (let line = 0; line < 2; line++) {
      const element = line === 0 ? lineOne : lineTwo;
      if (!element) continue;
      const fromLine = runAt.from[line];
      const toLine = runAt.to[line];
      const positionCount = runAt.positions[line];
      const offset = runAt.offsets[line];
      const fragment = document.createDocumentFragment();

      for (let position = 0; position < positionCount; position++) {
        const i = offset + position;
        const before = fromLine[position] ?? "";
        const after = position < toLine.length ? toLine[position] : null;

        if (before === " " && after === " ") {
          fragment.append(" ");
          continue;
        }

        if (elapsed < runAt.starts[i]) {
          if (before) fragment.append(before);
          continue;
        }

        if (after === null) {
          if (elapsed < runAt.ends[i]) {
            fragment.append(makeDud(runAt, i, now));
          }
          continue;
        }

        if (elapsed >= runAt.ends[i]) {
          fragment.append(after);
          continue;
        }

        fragment.append(makeDud(runAt, i, now));
      }

      element.replaceChildren(fragment);
    }
  }

  function makeDud(runAt: Run, index: number, now: number) {
    if (now - runAt.glyphAt[index] >= GLYPH_REFRESH_MS) {
      runAt.glyphs[index] = GLYPHS[Math.floor(Math.random() * GLYPHS.length)];
      runAt.glyphAt[index] = now;
    }
    const span = document.createElement("span");
    span.className = "dud";
    span.textContent = runAt.glyphs[index];
    return span;
  }

  function finish(now: number) {
    if (!run) return;
    run = null;
    holdEndsAt = now + holdMs;
    holdRemaining = holdMs;
    pausedHold = false;
    holdTimer = window.setTimeout(advance, holdMs);
  }

  function step(now: number) {
    raf = 0;
    if (!run || reduced || document.hidden) return;
    const elapsed = now - run.startedAt;
    render(run, elapsed, now);
    if (elapsed >= run.maxEnd) {
      finish(now);
      return;
    }
    raf = window.requestAnimationFrame(step);
  }

  function startDecode(from: [string, string], index: number) {
    stopCycle();
    if (!mounted || reduced || !active) {
      setStatic(index);
      return;
    }
    const next = linesOf(phrases[index]);
    phraseIndex = index;
    currentLines = next;
    run = buildRun(from, next);
    run.startedAt = performance.now();
    raf = window.requestAnimationFrame(step);
  }

  function advance() {
    holdTimer = 0;
    holdRemaining = 0;
    pausedHold = false;
    if (!active || reduced || document.hidden) return;
    startDecode(currentLines, (phraseIndex + 1) % phraseCount);
  }

  function onVisibilityChange() {
    if (!mounted || reduced) return;
    if (document.hidden) {
      pausedAt = performance.now();
      if (raf) {
        window.cancelAnimationFrame(raf);
        raf = 0;
      }
      if (holdTimer) {
        window.clearTimeout(holdTimer);
        holdTimer = 0;
        holdRemaining = Math.max(0, holdEndsAt - pausedAt);
        pausedHold = true;
      }
      return;
    }

    const now = performance.now();
    if (run) {
      run.startedAt += now - pausedAt;
      raf = window.requestAnimationFrame(step);
    } else if (pausedHold) {
      pausedHold = false;
      holdEndsAt = now + holdRemaining;
      holdTimer = window.setTimeout(advance, holdRemaining);
    }
  }

  onMount(() => {
    mounted = true;
    const media = window.matchMedia("(prefers-reduced-motion: reduce)");
    reduced = media.matches;
    lastPhrases = phrases;
    lastActive = active;

    const motionChanged = () => {
      reduced = media.matches;
      if (reduced) {
        stopCycle();
        setStatic(0);
      } else if (active) {
        startDecode(["", ""], 0);
      }
    };

    media.addEventListener("change", motionChanged);
    document.addEventListener("visibilitychange", onVisibilityChange);

    if (reduced || !active) setStatic(0);
    else startDecode(["", ""], 0);

    return () => {
      stopCycle();
      media.removeEventListener("change", motionChanged);
      document.removeEventListener("visibilitychange", onVisibilityChange);
    };
  });

  $: if (mounted && phrases !== lastPhrases) {
    lastPhrases = phrases;
    if (reduced || !active) setStatic(0);
    else startDecode(currentLines, 0);
  }

  $: if (mounted && active !== lastActive) {
    lastActive = active;
    if (!reduced) {
      if (active) startDecode(["", ""], phraseIndex);
      else {
        stopCycle();
        setStatic(phraseIndex);
      }
    }
  }
</script>

<div class="scrambleBlock" data-no-localize>
  <h1 class="headline">
    <span class="srOnly">{firstSentence}</span>
    <noscript><span class="staticHeadline" aria-hidden="true">{firstSentence}</span></noscript>
    <span class="lines" aria-hidden="true">
      <span class="line" bind:this={lineOne}></span>
      <span class="line accentLine" bind:this={lineTwo}></span>
    </span>
  </h1>
</div>

<style>
  .scrambleBlock {
    position: relative;
    width: 100%;
    max-width: 900px;
    margin: 0 auto;
    text-align: center;
  }

  .headline {
    max-width: 100%;
    margin: 0 auto;
    color: var(--color-text);
    font-family: var(--font-sans);
    font-size: clamp(44px, 5.5vw, 72px);
    font-weight: 650;
    letter-spacing: -0.065em;
    line-height: 0.96;
    text-wrap: balance;
  }

  .lines {
    display: block;
  }

  .line {
    display: block;
    min-height: 0.96em;
    white-space: pre-wrap;
    font-variant-ligatures: none;
  }

  .accentLine {
    position: relative;
    z-index: 0;
    width: fit-content;
    margin: 0 auto;
  }

  .accentLine::after {
    position: absolute;
    right: -0.05em;
    bottom: 0.02em;
    left: -0.04em;
    z-index: -1;
    height: 0.2em;
    border-radius: 3px;
    background: var(--color-accent);
    content: "";
    transform: rotate(-1deg);
  }

  .line :global(.dud) {
    color: var(--color-accent-text);
    opacity: 0.9;
  }



  .srOnly {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
    clip-path: inset(50%);
    white-space: nowrap;
    border: 0;
  }

  .staticHeadline {
    display: block;
    min-height: 0.96em;
    white-space: pre-wrap;
  }

  :global(.localeLong) .headline {
    font-size: clamp(42px, 5vw, 68px);
  }

  @media (max-width: 640px) {
    .headline {
      font-size: clamp(43px, 14vw, 64px);
    }

    :global(.localeLong) .headline {
      font-size: clamp(34px, 11.5vw, 52px);
    }

  }
</style>
