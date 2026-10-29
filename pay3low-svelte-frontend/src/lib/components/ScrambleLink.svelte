<script lang="ts">
  import { onMount } from "svelte";

  export let href: string;
  export let label: string;
  export let active = false;
  export let current: "page" | undefined = undefined;
  export let onNavigate: ((event: MouseEvent) => void) | undefined = undefined;

  const GLYPHS = "#%&*+=-_/\\<>[]{}?!$@0123456789";
  const STEP_MS = 22;
  const JITTER_MS = 60;
  const RESOLVE_MS = 120;
  const RESOLVE_JITTER_MS = 200;
  const GLYPH_REFRESH_MS = 50;

  type Run = {
    chars: string[];
    starts: number[];
    ends: number[];
    glyphs: string[];
    glyphAt: number[];
    maxEnd: number;
    startedAt: number;
  };

  let element: HTMLSpanElement;
  let mounted = false;
  let reduced = false;
  let running = false;
  let run: Run | null = null;
  let raf = 0;
  let currentLabel = label;

  function setLabel(value: string) {
    currentLabel = value;
    if (element) element.textContent = value;
  }

  function makeRun(value: string): Run {
    const chars = Array.from(value);
    const starts = new Array<number>(chars.length);
    const ends = new Array<number>(chars.length);
    let maxEnd = 0;
    for (let i = 0; i < chars.length; i++) {
      const start = i * STEP_MS + Math.random() * JITTER_MS;
      const end = start + RESOLVE_MS + Math.random() * RESOLVE_JITTER_MS;
      starts[i] = start;
      ends[i] = end;
      if (end > maxEnd) maxEnd = end;
    }
    return {
      chars,
      starts,
      ends,
      glyphs: new Array<string>(chars.length).fill(GLYPHS[0]),
      glyphAt: new Array<number>(chars.length).fill(0),
      maxEnd,
      startedAt: 0,
    };
  }

  function render(runAt: Run, elapsed: number, now: number) {
    if (!element) return;
    const fragment = document.createDocumentFragment();
    for (let i = 0; i < runAt.chars.length; i++) {
      const char = runAt.chars[i];
      if (elapsed < runAt.starts[i]) {
        fragment.append(char);
        continue;
      }
      if (elapsed >= runAt.ends[i]) {
        fragment.append(char);
        continue;
      }
      if (now - runAt.glyphAt[i] >= GLYPH_REFRESH_MS) {
        runAt.glyphs[i] = GLYPHS[Math.floor(Math.random() * GLYPHS.length)];
        runAt.glyphAt[i] = now;
      }
      const span = document.createElement("span");
      span.className = "dud";
      span.textContent = runAt.glyphs[i];
      fragment.append(span);
    }
    element.replaceChildren(fragment);
  }

  function step(now: number) {
    raf = 0;
    if (!run || reduced || !running) return;
    const elapsed = now - run.startedAt;
    render(run, elapsed, now);
    if (elapsed >= run.maxEnd) {
      const finalLabel = run.chars.join("");
      run = null;
      running = false;
      setLabel(finalLabel);
      return;
    }
    raf = window.requestAnimationFrame(step);
  }

  function trigger() {
    if (!mounted || reduced || running || !label) return;
    running = true;
    run = makeRun(label);
    run.startedAt = performance.now();
    raf = window.requestAnimationFrame(step);
  }

  function stop() {
    if (raf) window.cancelAnimationFrame(raf);
    raf = 0;
    run = null;
    running = false;
  }

  onMount(() => {
    mounted = true;
    const media = window.matchMedia("(prefers-reduced-motion: reduce)");
    reduced = media.matches;
    const motionChanged = () => {
      reduced = media.matches;
      if (reduced) {
        stop();
        setLabel(label);
      }
    };
    media.addEventListener("change", motionChanged);
    setLabel(label);
    return () => {
      stop();
      media.removeEventListener("change", motionChanged);
    };
  });

  $: if (label !== currentLabel && !running) setLabel(label);
</script>

<a {href} class:active aria-current={current} aria-label={label} on:mouseenter={trigger} on:focus={trigger} on:click={(event) => onNavigate?.(event)}>
  <span class="scrambleLabel" aria-hidden="true" bind:this={element}>{label}</span>
</a>

<style>
  .scrambleLabel {
    display: inline-block;
    white-space: pre;
    font-variant-ligatures: none;
  }

  .scrambleLabel :global(.dud) {
    color: var(--color-accent-text);
    opacity: 0.9;
  }
</style>
