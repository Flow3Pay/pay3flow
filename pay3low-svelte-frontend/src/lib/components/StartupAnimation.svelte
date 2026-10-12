<script lang="ts">
  import { onMount, tick } from "svelte";
  import { locale, t } from "$lib/i18n";
  import { otcCopy } from "$lib/otc/copy";
  export let mode: "swap" | "otc";
  export let onComplete: () => void;
  let overlay: HTMLDivElement;
  let introStarted = false;
  $: firstHeadline = headlineWords(t("Move money.", {}, $locale));
  $: secondHeadline = headlineWords(t("Keep more.", {}, $locale));
  $: otcHeadline = otcCopy($locale);
  function headlineWords(value: string) {
    const match = value.match(/^(\S+)\s+(.+?)([.!։。؟]+)$/u);
    return match ? { first: match[1], second: match[2], punctuation: match[3] } : { first: value, second: "", punctuation: "" };
  }
  function animationFinished(event: AnimationEvent) {
    if (event.animationName.endsWith("introDock") || event.animationName.endsWith("introOtcOut")) onComplete();
  }
  onMount(() => {
    let disposed = false;
    let frame: number | undefined, timer: number | undefined;
    const media = window.matchMedia("(prefers-reduced-motion: reduce)");
    const motionChanged = () => { if (media.matches) onComplete(); };
    const positionIntro = () => {
      if (mode !== "swap") return;
      const heading = document.querySelector(".hero h1")?.getBoundingClientRect();
      if (!heading) return;
      overlay.style.setProperty("--intro-x", `${heading.left + heading.width / 2 - window.innerWidth / 2}px`);
      overlay.style.setProperty("--intro-y", `${heading.top + heading.height / 2 - window.innerHeight / 2}px`);
    };
    document.documentElement.classList.add("introPlaying");
    window.addEventListener("resize", positionIntro, { passive: true });
    media.addEventListener("change", motionChanged);
    void tick().then(() => {
      if (disposed) return;
      positionIntro();
      frame = window.requestAnimationFrame(() => {
        if (disposed) return;
        introStarted = true;
        timer = window.setTimeout(onComplete, 5000);
      });
    });
    return () => {
      disposed = true;
      document.documentElement.classList.remove("introPlaying");
      window.removeEventListener("resize", positionIntro);
      media.removeEventListener("change", motionChanged);
      if (frame !== undefined) window.cancelAnimationFrame(frame);
      if (timer !== undefined) window.clearTimeout(timer);
    };
  });
</script>

<div class="introOverlay" class:introStarted class:localeLong={$locale !== "en"} class:otcIntro={mode === "otc"} bind:this={overlay} aria-hidden="true" data-startup-mode={mode}>
  <p class="introTitle" on:animationend={animationFinished}>
    {#if mode === "otc"}
      <span class="introClip"><span class="introWord introWordOtcLead">{otcHeadline.titleLead.trimEnd()}</span></span>{' '}<span class="introEmphasis"><span class="introClip"><span class="introWord introWordOtcAccent">{otcHeadline.titleAccent}</span></span><img class="introMarker" src="/icons/ui/marker-down-right.svg" alt="" width="512" height="512" aria-hidden="true" /></span>
    {:else}
      <span class="introClip"><span class="introWord introWordMove">{firstHeadline.first}</span></span>{' '}<span class="introSecondWithDot"><span class="introClip"><span class="introWord introWordMoney">{firstHeadline.second}</span></span><span class="introPunctuation introFirstPunctuation">{firstHeadline.punctuation}</span></span>{' '}<span class="introEmphasis"><span class="introClip"><span class="introWord introWordKeep">{secondHeadline.first}</span></span>{' '}<span class="introSecondWithDot"><span class="introClip"><span class="introWord introWordMore">{secondHeadline.second}</span></span><span class="introPunctuation introLastPunctuation">{secondHeadline.punctuation}</span></span><img class="introMarker" src="/icons/ui/marker-down-right.svg" alt="" width="512" height="512" aria-hidden="true" /></span>
    {/if}
  </p>
</div>

<style>
.introOverlay { pointer-events: none; position: fixed; inset: 0; z-index: 1000; overflow: hidden; background: var(--shell-gradient); }
.introTitle { position: absolute; top: 50%; left: 50%; width: min(900px, calc(100vw - 48px)); margin: 0; color: var(--color-text); font-size: clamp(44px, 5.5vw, 72px); font-weight: 650; letter-spacing: -.065em; line-height: .96; text-align: center; transform: translate(-50%, -50%) scale(1.2); }
.introStarted .introTitle { animation: introDock .68s 3.08s cubic-bezier(.22, 1, .36, 1) both; }
.introClip { display: inline-block; clip-path: inset(-.12em -.16em -.18em -.16em); vertical-align: bottom; }
.introSecondWithDot { white-space: nowrap; }
.introWord { display: inline-block; transform: translateY(115%); opacity: 0; }
.introStarted .introWordMove { animation: introRise .48s .2s cubic-bezier(.22, 1, .36, 1) forwards; }
.introStarted .introWordMoney { animation: introRise .48s .7s cubic-bezier(.22, 1, .36, 1) forwards; }
.introStarted .introWordKeep { animation: introRise .48s 1.35s cubic-bezier(.22, 1, .36, 1) forwards; }
.introStarted .introWordMore { animation: introRise .48s 1.85s cubic-bezier(.22, 1, .36, 1) forwards; }
.introPunctuation { opacity: 0; }
.introStarted .introFirstPunctuation { animation: introDot .02s 1.18s linear forwards; }
.introStarted .introLastPunctuation { animation: introDot .02s 2.94s linear forwards; }
.introEmphasis { position: relative; z-index: 0; white-space: nowrap; }
.introEmphasis::after { position: absolute; right: -.05em; bottom: .02em; left: -.04em; z-index: -1; height: .2em; border-radius: 3px; background: var(--color-accent); content: ""; transform: rotate(-1deg) scaleX(0); transform-origin: left center; }
.introStarted .introEmphasis::after { animation: introUnderline .48s 2.42s cubic-bezier(.22, 1, .36, 1) forwards; }
.introMarker { position: absolute; bottom: -.9em; left: -.04em; z-index: 2; width: 1.02em; height: 1.02em; pointer-events: none; opacity: 0; }
.introStarted .introMarker { animation: introMarkerDraw .48s 2.42s cubic-bezier(.22, 1, .36, 1) both; }
:global(html[data-theme="dark"]) .introMarker { filter: brightness(0) invert(1); }
.localeLong .introTitle { font-size: clamp(42px, 5vw, 68px); }
@media (max-width: 640px) { .introTitle { width: calc(100vw - 24px); font-size: 44px; transform: translate(-50%, -50%) scale(1.08); } .localeLong .introTitle { font-size: 44px; transform: translate(-50%, -50%) scale(1); } .localeLong .introEmphasis { white-space: normal; } }
@keyframes introRise { to { opacity: 1; transform: translateY(0); } }
@keyframes introDot { to { opacity: 1; } }
@keyframes introUnderline { to { transform: rotate(-1deg) scaleX(1); } }
@keyframes introMarkerDraw { 0% { left: -.04em; opacity: 0; } 8%, 88% { opacity: 1; } 100% { left: calc(100% + .05em); opacity: 0; } }
@keyframes introDock { to { transform: translate(calc(-50% + var(--intro-x)), calc(-50% + var(--intro-y))) scale(1); } }

.introStarted.otcIntro .introTitle { animation: introOtcOut .68s 2.2s cubic-bezier(.22, 1, .36, 1) both; }
.introStarted .introWordOtcLead { animation: introRise .48s .2s cubic-bezier(.22, 1, .36, 1) forwards; }
.introStarted .introWordOtcAccent { animation: introRise .48s .7s cubic-bezier(.22, 1, .36, 1) forwards; }
.introStarted.otcIntro .introEmphasis::after { animation-delay: 1.25s; }
.introStarted.otcIntro .introMarker { animation-delay: 1.25s; }
@media (max-width: 640px) { .otcIntro .introTitle { font-size: clamp(30px, 8vw, 44px); } }
@keyframes introOtcOut { to { opacity: 0; transform: translate(-50%, -50%) scale(1); } }
@media (prefers-reduced-motion: reduce) { .introOverlay { display: none; } }

</style>
