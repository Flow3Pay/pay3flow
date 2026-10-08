<script lang="ts">
  import { onMount, tick } from "svelte";
  import { lockPageScroll } from "$lib/page-scroll-lock";
  import type { ProviderGuidance, RouteCandidate, ServiceLink } from "$lib/exchange";
  import { locale, t } from "$lib/i18n";
  import { buildRouteTutorial, tutorialMoney } from "$lib/route-tutorial";
  import { venueIcon } from "$lib/icons";
  import AdvertiserCard from "./AdvertiserCard.svelte";
  import ExternalReviews from "./ExternalReviews.svelte";
  import RouteExecutionPanel from "./RouteExecutionPanel.svelte";
  import InstructionScene from "./InstructionScene.svelte";

  export let route: RouteCandidate;
  export let venueNames: Record<string, string> = {};
  export let providerGuidance: Record<string, ProviderGuidance> = {};
  export let networkNames: Record<string, string> = {};
  export let onClose: () => void;
  export let onOpenService: (link: ServiceLink) => void = () => {};

  let page: HTMLDivElement;
  let heading: HTMLHeadingElement;
  let chapter = -1;
  let completed = 0;
  let frame = 0;
  let elapsed = 0;
  let playing = true;
  let reducedMotion = false;
  let documentVisible = true;
  let mounted = false;
  const FRAME_DURATION = 6500;
  $: language = $locale;
  $: copy = (key: string, params: Record<string, string | number> = {}) => t(key, params, language);
  $: steps = buildRouteTutorial(route, venueNames, providerGuidance, networkNames, copy);
  $: step = steps[chapter];
  $: finished = steps.length > 0 && chapter === steps.length;
  $: activeFrame = step?.frames[frame];
  $: storageKey = `pay3flow.tutorial.v1.${route.route_id}`;
  $: if (mounted) remember(storageKey, chapter, completed);

  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return { destroy() { node.remove(); } };
  }
  function remember(key: string, active: number, done: number) {
    try { sessionStorage.setItem(key, JSON.stringify({ chapter: active, completed: done, steps: steps.map((item) => item.id) })); } catch { /* Optional in private browsing. */ }
  }
  async function navigate(next: number) {
    chapter = next;
    frame = 0;
    elapsed = 0;
    playing = !reducedMotion;
    await tick();
    page?.scrollTo({ top: 0, behavior: "instant" });
    heading?.focus({ preventScroll: true });
  }
  function confirmStep() {
    completed = Math.max(completed, chapter + 1);
    void navigate(chapter + 1);
  }
  function selectFrame(index: number) {
    frame = index;
    elapsed = 0;
    playing = false;
  }
  function replay() { frame = 0; elapsed = 0; playing = !reducedMotion; }
  function togglePlayback() {
    if (!playing && frame === (step?.frames.length ?? 0) - 1 && elapsed >= FRAME_DURATION) replay();
    else playing = !playing;
  }
  function reviewSource(kind: string) {
    return ({ trustpilot: "Trustpilot", trustscores: "TrustScores", otzovik: "Otzovik", forum: "Bits.media", bestchange: "BestChange", "yandex-maps": "Yandex Maps", rustore: "RuStore", provider: "Provider" } as Record<string, string>)[kind] ?? kind;
  }
  function warn(warning: string) {
    const dry = warning.match(/^Live dry quote from (.+); execution and wallet compatibility are not verified\.$/);
    if (dry) return copy("Live dry quote from {provider}; execution and wallet compatibility are not verified.", { provider: dry[1] });
    const exchanger = warning.match(/^Quoted exchanger: (.+)\.$/);
    return exchanger ? copy("Quoted exchanger: {description}.", { description: exchanger[1] }) : copy(warning);
  }
  onMount(() => {
    const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const unlock = lockPageScroll();
    // Isolate the full-screen guide while retaining the route search beneath it.
    const siblings: Array<{ element: HTMLElement; inert: boolean }> = [];
    let branch: HTMLElement = page;
    while (branch.parentElement) {
      for (const child of branch.parentElement.children) {
        if (child !== branch && child instanceof HTMLElement && !["SCRIPT", "STYLE", "LINK"].includes(child.tagName)) {
          siblings.push({ element: child, inert: child.inert }); child.inert = true;
        }
      }
      branch = branch.parentElement;
      if (branch === document.body) break;
    }
    try {
      const saved = JSON.parse(sessionStorage.getItem(storageKey) ?? "null");
      if (saved && JSON.stringify(saved.steps) === JSON.stringify(steps.map((item) => item.id)) && Number.isInteger(saved.completed) && Number.isInteger(saved.chapter) && saved.completed >= 0 && saved.completed <= steps.length && saved.chapter >= -1 && saved.chapter <= saved.completed) {
        completed = saved.completed; chapter = saved.chapter;
      }
    } catch { /* A malformed or unavailable cache must not block the guide. */ }
    mounted = true;
    const media = matchMedia("(prefers-reduced-motion: reduce)");
    const motionChanged = () => { reducedMotion = media.matches; if (reducedMotion) playing = false; };
    motionChanged(); media.addEventListener("change", motionChanged);
    const visibilityChanged = () => documentVisible = !document.hidden;
    visibilityChanged(); document.addEventListener("visibilitychange", visibilityChanged);
    const keyboard = (event: KeyboardEvent) => {
      if (event.key === "Escape") { event.preventDefault(); onClose(); }
      if (event.key !== "Tab") return;
      const focusable = [...page.querySelectorAll<HTMLElement>('button:not(:disabled), a[href], input:not(:disabled), summary, [tabindex="0"]')].filter((item) => item.getClientRects().length > 0);
      const first = focusable[0], last = focusable.at(-1);
      if (event.shiftKey && (document.activeElement === first || document.activeElement === heading || document.activeElement === page)) { event.preventDefault(); last?.focus(); }
      else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
    };
    document.addEventListener("keydown", keyboard);
    heading?.focus({ preventScroll: true });
    let previousTick = performance.now();
    const timer = setInterval(() => {
      const now = performance.now(), delta = Math.min(now - previousTick, 500); previousTick = now;
      if (!playing || !documentVisible || !step) return;
      elapsed = Math.min(FRAME_DURATION, elapsed + delta);
      if (elapsed >= FRAME_DURATION) {
        if (frame < step.frames.length - 1) { frame += 1; elapsed = 0; }
        else playing = false;
      }
    }, 100);
    return () => {
      clearInterval(timer); unlock();
      siblings.forEach(({ element, inert }) => element.inert = inert);
      document.removeEventListener("keydown", keyboard);
      document.removeEventListener("visibilitychange", visibilityChanged);
      media.removeEventListener("change", motionChanged);
      previousFocus?.focus({ preventScroll: true });
    };
  });
</script>

<div class="guidePage" use:portal bind:this={page} role="dialog" aria-modal="true" aria-labelledby="route-instructions-title" tabindex="-1" data-testid="route-guide">
  <h2 id="route-instructions-title" class="srOnly">{copy("How to complete this exchange")}</h2>
  <header class="guideHeader">
    <div class="guideBrand"><span class="brandMark">↗</span><strong>pay3flow<span>.</span></strong><span class="brandDivider"></span><span class="guideLabel">{copy("Exchange guide")}</span></div>
    <button type="button" class="closeButton" on:click={onClose} aria-label={copy("Close instructions")}><span class="closeText">{copy("Back to routes")}</span><span>×</span></button>
  </header>
  <div class="guideLayout">
    <aside class="chapters">
      <span class="eyebrow">{copy("YOUR ROUTE")}</span>
      <div class="routePair">{route.source_currency}<span>↗</span>{route.target_currency ?? route.entry_asset}</div>
      <button type="button" class="overviewButton" class:current={chapter === -1} aria-current={chapter === -1 ? "step" : undefined} on:click={() => navigate(-1)}><span>◎</span>{copy("Before you begin")}</button>
      <ol aria-label={copy("Exchange steps")}>
        {#each steps as item, index}
          <li data-testid="instruction-step" class:complete={index < completed} class:active={chapter === index}>
            <button type="button" aria-label={copy("Step {number}: {title}", { number: index + 1, title: item.title })} disabled={index > completed} aria-current={chapter === index ? "step" : undefined} on:click={() => navigate(index)}>
              <span class="stepNumber">{index < completed ? "✓" : index + 1}</span><span class="chapterCopy"><strong>{item.title}</strong><small>{item.venue}</small></span>
            </button>
          </li>
        {/each}
      </ol>
      <div class="chapterProgress"><div><span>{copy("Your progress")}</span><strong>{completed}/{steps.length}</strong></div><div class="progressTrack"><span style={`width:${steps.length ? completed / steps.length * 100 : 0}%`}></span></div><p>{copy("Continue after completing each step on the platform.")}</p></div>
      <div class="sidebarNote"><span>↗</span><p>{copy("Watch. Do. Continue.")}</p></div>
    </aside>
    <main class="guideMain">
      {#key chapter}
        <div class="chapterEntrance">
          <div class="chapterHeading"><span class="eyebrow">{finished ? copy("GUIDE COMPLETED") : chapter === -1 ? copy("LET’S GET YOU THERE") : copy("STEP {current} OF {total}", { current: chapter + 1, total: steps.length })}</span><span class="chapterType">{step ? copy({ buy: "Buy", sell: "Sell", swap: "Exchange", transfer: "Transfer" }[step.kind]) : copy("Step by step")}</span></div>
          <h1 bind:this={heading} tabindex="-1">{finished ? copy("Every step. Done.") : step?.title ?? copy("A clear route. At your pace.")}</h1>
          <p class="lead">{finished ? copy("You have confirmed every step. Check the final balance in your bank, wallet or exchange account.") : step?.summary ?? copy("First, see how your exchange works. Then follow the animated walkthrough, one step at a time.")}</p>
          <div class="chapterContent">
            <div class="visualColumn">
              <InstructionScene {route} {steps} {step} {frame} {finished} playing={playing && documentVisible && !reducedMotion} />
              {#if step}
                <div class="playerControls">
                  <button type="button" on:click={togglePlayback} aria-label={copy(playing ? "Pause walkthrough" : "Play walkthrough")} aria-pressed={playing}><span>{playing ? "Ⅱ" : "▶"}</span></button>
                  <div class="playerTimeline" aria-label={copy("Walkthrough scenes")}>{#each step.frames as item, index}<button type="button" aria-label={copy("Scene {number}: {title}", { number: index + 1, title: item.title })} aria-pressed={index === frame} on:click={() => selectFrame(index)}><span style={`width:${index < frame ? 100 : index === frame ? Math.max(4, elapsed / FRAME_DURATION * 100) : 0}%`}></span></button>{/each}</div>
                  <span class="sceneCount">{String(frame + 1).padStart(2, "0")} / {String(step.frames.length).padStart(2, "0")}</span>
                  <button type="button" on:click={replay} aria-label={copy("Replay walkthrough")}>↻</button>
                </div>
              {:else}<div class="visualCaption"><span>✦</span>{copy(finished ? "Confirmed by you" : "An animated guide for your selected route")}</div>{/if}
            </div>
            <section class="explanation" aria-label={copy("Step instructions")}>
              {#if step}
                <span class="eyebrow">{copy("WHAT TO DO")}</span>
                <div class="frameList">{#each step.frames as item, index}<button type="button" class:selected={index === frame} aria-pressed={index === frame} on:click={() => selectFrame(index)}><span class="frameNumber">{String(index + 1).padStart(2, "0")}</span><span><strong>{item.title}</strong><span class="frameText">{item.text}</span></span>{#if index === frame}<span class="frameIndicator">←</span>{/if}</button>{/each}</div>
                <div class="checkpoint"><span>✓</span><div><strong>{copy("Before moving on")}</strong><p>{step.checkpoint}</p></div></div>
              {:else if finished}
                <span class="eyebrow">{copy("YOUR CHECKLIST")}</span>
                <div class="finishChecklist">{#each steps as item}<div><span>✓</span><p>{item.title}</p></div>{/each}</div>
                <div class="checkpoint"><span>◎</span><div><strong>{copy("Confirmed by you")}</strong><p>{copy("This checklist records your confirmations. It does not verify payments or balances.")}</p></div></div>
              {:else}
                <span class="eyebrow">{copy("THE PLAN")}</span>
                <div class="planItem"><span>01</span><div><strong>{copy("See the whole picture")}</strong><p>{copy("Your route is split into {count} steps. We explain each action before you do it.", { count: steps.length })}</p></div></div>
                <div class="planItem"><span>02</span><div><strong>{copy("Follow the demonstration")}</strong><p>{copy("Watch the highlights, pause whenever you need, and open the actual platform from the step.")}</p></div></div>
                <div class="planItem"><span>03</span><div><strong>{copy("Confirm, then continue")}</strong><p>{copy("Once the payment or balance arrives, press “Done, continue”. You decide when to move on.")}</p></div></div>
                <div class="estimate"><span>{copy("Estimated output:")}</span><strong>{tutorialMoney(route.target_amount_minor, route.target_currency, route.target_amount)}</strong><small>{copy("Check the current quote on the platform before exchanging.")}</small></div>
              {/if}
            </section>
          </div>
          {#if step}
            <section class="realAction" aria-label={copy("On the actual platform")}>
              <div class="realActionHeading"><div><img src={venueIcon(step.provider)} alt="" /><div><span class="eyebrow">{copy("ON THE ACTUAL PLATFORM")}</span><h2>{step.venue}</h2></div></div><span>{copy("Complete this step here")}</span></div>
              {#if step.offer}<AdvertiserCard offer={step.offer} label={step.direct ? copy("Direct exchange on {venue}", { venue: step.venue }) : `${copy(step.kind === "buy" ? "Seller" : "Buyer")} ${copy("on {venue}", { venue: step.venue })}`} serviceLink={step.serviceLink} {venueNames} {onOpenService} />
              {:else if step.serviceLink}<button type="button" class="platformLink" on:click={() => onOpenService(step!.serviceLink!)}>{copy("Open {venue}", { venue: step.venue })}<span>↗</span></button>
              {:else if step.url}<a class="platformLink" href={step.url} target="_blank" rel="noreferrer noopener">{copy("Open {venue}", { venue: step.venue })}<span>↗</span></a>{/if}
              {#if step.pair}<p class="marketInfo">Spot · {step.pair}{#if step.rate}<span>{copy("Conversion rate {rate}", { rate: step.rate })}</span>{/if}</p>{/if}
              {#if step.execution}<RouteExecutionPanel {route} />{/if}
              {#if step.guide || step.notes.length}
                <details class="providerDetails" open={step.kind === "transfer" || Boolean(step.notes.length)}><summary>{copy("Details and platform instructions")}</summary><div>{#if step.guide}<p>{step.guide.description}</p>{/if}<ul>{#each [...step.guideSteps, ...step.notes] as note}<li>{note}</li>{/each}</ul>{#if step.guide?.links.length}<div class="guideLinks">{#each step.guide.links as link}<a href={link.url} target="_blank" rel="noreferrer noopener">{link.label} ↗</a>{/each}</div>{/if}</div></details>
              {/if}
              {#if step.direct && step.guide?.review_sources?.[0]}
                {@const source = step.guide.review_sources[0]}
                <ExternalReviews provider={step.provider} sourceUrl={source.url} sourceName={reviewSource(source.kind)} />
              {:else if step.offer?.advertiser_profile_url && step.guide?.profile_reviews_available !== false}
                <ExternalReviews profileUrl={step.offer.advertiser_profile_url} sourceName={step.venue} />
              {:else if step.direct && step.url && step.guide?.profile_reviews_available !== false}
                <ExternalReviews profileUrl={step.url} sourceName={step.venue} />
              {/if}
            </section>
          {/if}
          {#if chapter === -1}
            <div class="routeNotice"><span>ⓘ</span><div><strong>{copy("Before you begin")}</strong><p>{copy("Rates, limits, and offers can change. Check the provider, payment details, and network before sending money. Pay3Flow does not create orders or move money.")}</p>{#if route.source_bank_fee_percent != null}<p>{copy("Bank fees:")} {route.source_payment_method ?? route.source_currency}: {route.source_bank_fee_percent}%</p>{/if}{#if route.target_bank_fee_percent != null}<p>{copy("Bank fees:")} {route.target_payment_method ?? route.target_currency}: {route.target_bank_fee_percent}%</p>{/if}{#each route.warnings ?? [] as warning}<p>{warn(warning)}</p>{/each}</div></div>
          {/if}
        </div>
      {/key}
    </main>
  </div>
  <footer class="guideFooter"><div><span class="footerDot"></span><span>{finished ? copy("All steps confirmed") : step ? copy("Only continue once you have completed this step.") : copy("You control every step")}</span></div><div class="footerActions">{#if chapter >= 0 && !finished}<button type="button" class="backButton" on:click={() => navigate(chapter - 1)}>← {copy("Back")}</button>{/if}{#if finished}<button type="button" class="backButton" on:click={() => navigate(0)}>{copy("Review steps")}</button><button type="button" class="continueButton" on:click={onClose} aria-label={copy("Back to routes")}>{copy("Back to routes")} <span aria-hidden="true">↗</span></button>{:else if step}<button type="button" class="continueButton" on:click={confirmStep} data-testid="confirm-instruction-step">{copy(chapter === steps.length - 1 ? "Done, finish" : "Done, continue")} <span>→</span></button>{:else}<button type="button" class="continueButton" disabled={!steps.length} on:click={() => navigate(0)} data-testid="start-guide">{copy("Let’s begin")} <span>→</span></button>{/if}</div></footer>
</div>

<style>
  .guidePage { position: fixed; inset: 0; z-index: 1200; overflow-y: auto; overscroll-behavior: contain; background: var(--color-bg); color: var(--color-text); animation: pageIn .35s ease both; }
  .srOnly { position: absolute; width: 1px; height: 1px; padding: 0; overflow: hidden; clip: rect(0,0,0,0); white-space: nowrap; border: 0; }
  .guideHeader { display: flex; align-items: center; justify-content: space-between; gap: 20px; padding: 23px 40px; border-bottom: 1px solid var(--color-border); }
  .guideBrand { display: flex; align-items: center; gap: 10px; }.brandMark { display: grid; place-items: center; width: 31px; height: 31px; border-radius: 10px; background: var(--color-accent); color: #152016; font-size: 25px; }.guideBrand > strong { font-size: 23px; font-weight: 800; letter-spacing: -1.3px; }.guideBrand > strong > span { color: var(--color-accent-text); }.brandDivider { height: 22px; width: 1px; background: var(--color-border); margin: 0 9px; }.guideLabel { font-size: 12px; color: var(--color-text-soft); }
  .closeButton { display: flex; align-items: center; gap: 17px; color: var(--color-text-soft); font-size: 12px; }.closeButton > span:last-child { display: grid; place-items: center; width: 35px; height: 35px; border: 1px solid var(--color-border); border-radius: 50%; font-size: 23px; }.closeButton:hover { color: var(--color-text); }
  .guideLayout { display: grid; grid-template-columns: 246px minmax(0, 1fr); width: min(1440px, 100%); margin: 0 auto; }
  .chapters { padding: 37px 24px 120px 40px; border-right: 1px solid var(--color-border); min-width: 0; }
  .eyebrow { font: 10px var(--font-mono); letter-spacing: .09em; color: var(--color-text-soft); }
  .routePair { display: flex; align-items: center; gap: 13px; margin-top: 15px; font-size: 24px; font-weight: 800; letter-spacing: -.05em; }.routePair > span { font-size: 20px; color: var(--color-text-soft); }
  .overviewButton { display: flex; align-items: center; gap: 13px; width: 100%; padding: 12px 8px; margin-top: 31px; border-radius: 8px; text-align: left; font-size: 12px; font-weight: 650; color: var(--color-text-soft); }.overviewButton > span { font-size: 20px; }.overviewButton.current { color: var(--color-text); background: var(--color-panel-soft); }
  .chapters ol { list-style: none; display: grid; gap: 9px; margin-top: 19px; }
  .chapters li { position: relative; }.chapters li:not(:last-child)::after { content: ""; position: absolute; left: 20px; top: 44px; bottom: -13px; width: 1px; background: var(--color-border); }
  .chapters li button { display: flex; align-items: flex-start; gap: 12px; width: 100%; padding: 10px 6px; text-align: left; border-radius: 9px; }.chapters li button:disabled { cursor: default; color: var(--color-text-soft); }.chapters li.active button { background: var(--color-accent-soft); }
  .stepNumber { display: grid; place-items: center; flex: 0 0 28px; height: 28px; border: 1px solid var(--color-border-strong); border-radius: 50%; font: 11px var(--font-mono); }.active .stepNumber, .complete .stepNumber { background: var(--color-accent); border-color: var(--color-accent); color: #182414; }
  .chapterCopy { display: grid; gap: 6px; padding-top: 3px; min-width: 0; }.chapterCopy strong { font-size: 11px; font-weight: 700; line-height: 1.55; overflow-wrap: anywhere; }.chapterCopy small { font-size: 10px; color: var(--color-text-soft); }
  .chapterProgress { margin-top: 36px; padding-top: 24px; border-top: 1px solid var(--color-border); }.chapterProgress > div:first-child { display: flex; justify-content: space-between; font-size: 10px; color: var(--color-text-soft); }.chapterProgress strong { font: 11px var(--font-mono); color: var(--color-text); }.progressTrack { margin-top: 12px; height: 3px; border-radius: 3px; background: var(--color-border); overflow: hidden; }.progressTrack > span { display: block; height: 100%; background: var(--color-accent-strong); transition: width .6s ease; }.chapterProgress p { margin-top: 12px; color: var(--color-text-soft); font-size: 10px; line-height: 1.8; }.sidebarNote { display: flex; align-items: center; gap: 12px; margin-top: 70px; color: var(--color-text-faint); font-size: 11px; }.sidebarNote > span { font-size: 27px; }
  .guideMain { min-width: 0; padding: 38px 42px 120px; }.chapterEntrance { animation: chapterIn .5s cubic-bezier(.22,1,.36,1) both; }.chapterHeading { display: flex; align-items: center; justify-content: space-between; gap: 15px; }.chapterType { border: 1px solid var(--color-border); border-radius: 999px; padding: 5px 10px; font-size: 10px; color: var(--color-text-soft); }
  h1 { font-size: clamp(30px, 3.3vw, 49px); line-height: 1.15; font-weight: 750; letter-spacing: -.055em; margin-top: 17px; max-width: 960px; }h1:focus { outline: none; }.lead { max-width: 710px; color: var(--color-text-soft); font-size: 13px; line-height: 1.8; margin-top: 14px; }
  .chapterContent { display: grid; grid-template-columns: minmax(0, 1.1fr) minmax(260px, .85fr); gap: 33px; margin-top: 31px; align-items: start; }.visualColumn { min-width: 0; }.explanation { padding-top: 7px; min-width: 0; }
  .planItem { display: flex; align-items: flex-start; gap: 14px; margin-top: 25px; }.planItem > span { padding-top: 2px; color: var(--color-text-faint); font: 11px var(--font-mono); }.planItem strong { font-size: 14px; font-weight: 750; letter-spacing: -.02em; }.planItem p { color: var(--color-text-soft); font-size: 12px; line-height: 1.85; margin-top: 7px; }
  .estimate { display: grid; gap: 10px; margin-top: 31px; padding: 20px; border: 1px solid var(--color-border); border-radius: 13px; background: var(--color-paper); }.estimate > span { color: var(--color-text-soft); font-size: 11px; }.estimate strong { font-size: 27px; font-weight: 750; letter-spacing: -.04em; overflow-wrap: anywhere; }.estimate small { font-size: 10px; color: var(--color-text-soft); line-height: 1.7; }
  .visualCaption { display: flex; align-items: center; justify-content: center; gap: 7px; margin-top: 16px; font-size: 10px; color: var(--color-text-soft); }.visualCaption > span { color: var(--color-accent-text); }
  .frameList { display: grid; gap: 7px; margin-top: 15px; }.frameList button { display: flex; align-items: flex-start; width: 100%; gap: 11px; text-align: left; padding: 13px 12px; border: 1px solid transparent; border-radius: 11px; transition: background .3s, border-color .3s; }.frameList button.selected { border-color: var(--color-border); background: var(--color-paper); }.frameNumber { flex: 0 0 22px; margin-top: 2px; font: 10px var(--font-mono); color: var(--color-text-faint); }.selected .frameNumber { color: var(--color-accent-text); }.frameList strong { font-size: 12px; font-weight: 750; line-height: 1.5; }.frameText { display: block; color: var(--color-text-soft); font-size: 11px; line-height: 1.8; margin-top: 5px; }.frameIndicator { font-size: 15px; color: var(--color-accent-text); margin-left: auto; }
  .checkpoint { display: flex; gap: 10px; padding: 17px; background: var(--color-accent-soft); border: 1px solid var(--color-border); border-radius: 11px; margin-top: 19px; }.checkpoint > span { color: var(--color-accent-text); font-size: 18px; }.checkpoint strong { font-size: 11px; }.checkpoint p { font-size: 11px; line-height: 1.8; margin-top: 5px; color: var(--color-text-soft); }
  .playerControls { display: flex; align-items: center; gap: 10px; padding: 8px 4px; margin-top: 5px; }.playerControls > button { font-size: 17px; color: var(--color-text-soft); width: 32px; height: 32px; }.playerTimeline { display: flex; gap: 4px; flex: 1; }.playerTimeline button { position: relative; flex: 1; min-width: 0; height: 32px; background: transparent; }.playerTimeline button::before { content: ""; position: absolute; left: 0; right: 0; height: 3px; top: 15px; border-radius: 3px; background: var(--color-border); }.playerTimeline button > span { position: absolute; top: 15px; left: 0; height: 3px; border-radius: 3px; background: var(--color-accent-text); transition: width .1s linear; }.sceneCount { white-space: nowrap; font: 9px var(--font-mono); color: var(--color-text-soft); }
  .realAction { padding: 22px; margin-top: 28px; border: 1px solid var(--color-border); border-radius: 16px; background: var(--color-paper); }.realActionHeading { display: flex; justify-content: space-between; align-items: center; gap: 20px; margin-bottom: 14px; }.realActionHeading > div { display: flex; align-items: center; gap: 11px; }.realActionHeading img { width: 32px; height: 32px; border-radius: 50%; }.realActionHeading h2 { margin-top: 5px; font-size: 16px; }.realActionHeading > span { color: var(--color-text-soft); font-size: 11px; }.platformLink { display: flex; align-items: center; justify-content: space-between; gap: 18px; width: fit-content; min-height: 44px; padding: 0 16px; background: var(--color-accent); color: #152016; border-radius: 10px; font-size: 12px; font-weight: 750; }.platformLink:hover { background: #c3ff22; }.platformLink > span { font-size: 19px; }.marketInfo { margin-top: 14px; font-size: 12px; font-family: var(--font-mono); }.marketInfo > span { display: block; font-family: var(--font-sans); margin-top: 5px; color: var(--color-text-soft); }
  .providerDetails { margin-top: 20px; padding-top: 15px; border-top: 1px solid var(--color-border); font-size: 12px; line-height: 1.85; }.providerDetails summary { cursor: pointer; font-weight: 700; }.providerDetails > div { margin-top: 13px; color: var(--color-text-soft); }.providerDetails ul { padding-left: 18px; display: grid; gap: 7px; margin-top: 10px; }.guideLinks { display: flex; flex-wrap: wrap; gap: 15px; margin-top: 13px; }.guideLinks a { text-decoration: underline; text-underline-offset: 3px; }
  .routeNotice { display: flex; align-items: flex-start; gap: 12px; margin-top: 30px; padding-top: 20px; border-top: 1px solid var(--color-border); color: var(--color-text-soft); font-size: 11px; line-height: 1.9; }.routeNotice > span { font-size: 18px; }.routeNotice strong { color: var(--color-text); font-size: 11px; }.routeNotice p { margin-top: 5px; }.finishChecklist { margin-top: 22px; display: grid; gap: 17px; }.finishChecklist > div { display: flex; gap: 11px; font-size: 12px; line-height: 1.7; }.finishChecklist > div > span { color: var(--color-accent-text); }
  .guideFooter { position: fixed; left: 0; right: 0; bottom: 0; z-index: 5; display: flex; justify-content: space-between; align-items: center; gap: 22px; padding: 17px 40px; background: var(--color-paper); border-top: 1px solid var(--color-border); }.guideFooter > div:first-child { display: flex; align-items: center; gap: 8px; color: var(--color-text-soft); font-size: 11px; }.footerDot { width: 5px; height: 5px; border-radius: 50%; background: var(--color-accent-text); flex-shrink: 0; }.footerActions { display: flex; align-items: center; gap: 24px; }.backButton { font-size: 12px; color: var(--color-text-soft); }.continueButton { display: flex; align-items: center; justify-content: space-between; gap: 35px; min-height: 47px; padding: 0 21px; border-radius: 10px; background: var(--color-accent); color: #152016; font-size: 13px; font-weight: 800; }.continueButton > span { font-size: 22px; font-weight: 400; }.continueButton:hover { background: #c3ff22; }.continueButton:disabled { opacity: .5; cursor: default; }
  @keyframes pageIn { from { opacity: 0; transform: translateY(12px); } }@keyframes chapterIn { from { opacity: 0; transform: translateY(14px); } }
  @media (min-width: 761px) { .guideFooter { position: fixed; left: 0; right: 0; }.guideLayout { min-height: calc(100dvh - 82px); } }
  @media (max-height: 820px) and (min-width: 901px) { .guideHeader { padding-top: 16px; padding-bottom: 16px; }.guideMain { padding-top: 25px; }h1 { font-size: 38px; margin-top: 12px; }.lead { margin-top: 10px; }.chapterContent { margin-top: 24px; } }
  @media (max-width: 1100px) { .guideLayout { grid-template-columns: 205px minmax(0,1fr); }.chapters { padding-left: 24px; padding-right: 18px; }.guideMain { padding-left: 27px; padding-right: 27px; }.chapterContent { gap: 20px; grid-template-columns: minmax(0,1fr) minmax(235px,.85fr); }.guideHeader { padding-left: 24px; padding-right: 24px; }.guideFooter { padding-left: 24px; padding-right: 24px; } }
  @media (max-width: 900px) and (min-width: 761px) { .guideLayout { grid-template-columns: 170px minmax(0,1fr); }.chapters { padding-left: 18px; padding-right: 12px; }.chapterContent { grid-template-columns: minmax(0,1fr); }.guideMain { padding-left: 25px; padding-right: 25px; }.frameList { grid-template-columns: 1fr 1fr; }.sidebarNote { display: none; } }
  @media (max-width: 760px) {
    .guideHeader { padding: 14px 18px; }.guideBrand { gap: 7px; }.guideBrand > strong { font-size: 21px; }.brandMark { width: 27px; height: 27px; font-size: 23px; border-radius: 8px; }.brandDivider, .guideLabel, .closeText { display: none; }.closeButton > span:last-child { width: 32px; height: 32px; }
    .guideLayout { display: block; }.chapters { padding: 15px 18px; border-right: 0; border-bottom: 1px solid var(--color-border); }.chapters > .eyebrow, .routePair, .chapterProgress, .sidebarNote, .chapterCopy { display: none; }.chapters { display: flex; align-items: center; gap: 12px; overflow-x: auto; }.overviewButton { flex: 0 0 auto; width: auto; margin: 0; padding: 7px 10px; font-size: 11px; }.chapters ol { display: flex; gap: 8px; margin: 0; }.chapters li:not(:last-child)::after { display: none; }.chapters li button { padding: 5px; min-width: 44px; min-height: 44px; display: grid; place-items: center; }.stepNumber { width: 29px; height: 29px; }.guideMain { padding: 26px 18px 100px; }h1 { margin-top: 13px; font-size: 33px; }.lead { font-size: 12px; line-height: 1.85; margin-top: 11px; }.chapterContent { grid-template-columns: minmax(0,1fr); gap: 25px; margin-top: 23px; }.chapterType { font-size: 9px; }.planItem { margin-top: 21px; }.estimate { margin-top: 24px; }.frameList button { padding: 12px 10px; }.frameText { font-size: 12px; }.frameList strong { font-size: 13px; }.checkpoint p { font-size: 12px; }.realAction { padding: 17px; margin-top: 24px; }.realActionHeading > span { display: none; }.realActionHeading .eyebrow { font-size: 9px; }.routeNotice { font-size: 11px; }
    .guideFooter { padding: 12px 18px max(12px, env(safe-area-inset-bottom)); gap: 12px; }.guideFooter > div:first-child { display: none; }.footerActions { justify-content: space-between; width: 100%; gap: 14px; }.continueButton { min-height: 48px; flex: 1; justify-content: center; gap: 23px; }.backButton { flex: 0 0 auto; font-size: 11px; }.playerControls > button { min-width: 44px; min-height: 44px; }.playerTimeline button { min-width: 0; }
  }
  @media (prefers-reduced-motion: reduce) { .guidePage, .chapterEntrance { animation: none; } }
</style>
