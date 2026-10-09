<script lang="ts">
  import { onMount, tick } from "svelte";
  import type { ProviderGuidance, RouteCandidate, ServiceLink } from "$lib/exchange";
  import { locale, t } from "$lib/i18n";
  import { buildRouteTutorial, tutorialMoney } from "$lib/route-tutorial";
  import { fiatFlagUrl } from "$lib/currency-flags";
  import { assetIcon, venueIcon } from "$lib/icons";
  import { guideSharePath } from "$lib/guide-link";
  // import AdvertiserCard from "./AdvertiserCard.svelte";
  import ExternalReviews from "./ExternalReviews.svelte";
  import RouteExecutionPanel from "./RouteExecutionPanel.svelte";
  import { wallets } from "$lib/wallet-session";
  import { walletFamily } from "$lib/wallet-execution";
  import InstructionScene from "./InstructionScene.svelte";
  import { spotTrade } from "$lib/guides/spot/frames";

  export let route: RouteCandidate;
  export let venueNames: Record<string, string> = {};
  export let venueUrls: Record<string, string> = {};
  export let providerGuidance: Record<string, ProviderGuidance> = {};
  export let networkNames: Record<string, string> = {};
  export let onClose: () => void;
  // svelte-ignore export_let_unused
  export let onOpenService: (link: ServiceLink) => void = () => {};

  let tab: "guide" | "reviews" = "guide";
  let shareCopied = false;
  let shareFallback = "";
  type ReviewSource = { key: string; venue: string; provider: string | null; profileUrl: string | null; sourceUrl: string; sourceName: string };
  $: reviewSources = steps.flatMap<ReviewSource>(item => {
    if (item.guide?.profile_reviews_available === false && !item.guide.review_sources?.length) return [];
    const source = item.guide?.review_sources?.[0];
    if (item.direct && source) return [{ key: item.provider, venue: item.venue, provider: item.provider, profileUrl: null, sourceUrl: source.url, sourceName: reviewSource(source.kind) }];
    const profile = item.offer?.advertiser_profile_url ?? (item.direct ? item.url : null);
    return profile ? [{ key: profile, venue: item.offer && !item.direct ? `${item.venue} · ${item.offer.advertiser.nickname}` : item.venue, provider: null, profileUrl: profile, sourceUrl: profile, sourceName: item.venue }] : [];
  }).filter((item, index, all) => all.findIndex(other => other.key === item.key) === index);
  let heading: HTMLHeadingElement;
  let chapter = -1;
  let completed = 0;
  let frame = 0;
  let elapsed = 0;
  let playing = true;
  let reducedMotion = false;
  let documentVisible = true;
  let sceneVisible = true;
  function sceneVisibility(node: HTMLElement) {
    const observer = new IntersectionObserver(entries => { sceneVisible = entries[0]?.isIntersecting ?? false; }, { rootMargin: "40px" });
    observer.observe(node);
    return { destroy() { observer.disconnect(); } };
  }
  $: frameDuration = step?.offer && !step.direct && ["bybit", "mexc", "binance"].includes(step.provider.toLowerCase()) ? 5200 : 6500;
  $: language = $locale;
  $: copy = (key: string, params: Record<string, string | number> = {}) => t(key, params, language);
  $: steps = buildRouteTutorial(route, venueNames, providerGuidance, networkNames, copy);
  $: step = steps[chapter];
  $: spot = step?.kind === "swap" ? spotTrade(step.pair, step.from, step.to) : null;
  $: sourceFamily = walletFamily(route.execution?.from_asset.split("@", 2)[1] ?? "");
  $: walletSwap = Boolean(step?.execution && route.execution && sourceFamily && $wallets[sourceFamily]);
  $: walletProviderStep = Boolean(step?.execution && ["symbiosis", "near-intents", "cow-swap"].includes(step.provider));
  $: walletProviderUrl = step?.url ?? step?.guide?.links.find(link => /^Open\b/i.test(link.label))?.url ?? (step && venueUrls[step.provider.toLowerCase()]);
  $: finished = steps.length > 0 && chapter === steps.length;
  $: titleIncludesVenue = Boolean(step && step.title.endsWith(step.venue));
  $: headingTitle = titleIncludesVenue && step ? step.title.slice(0, -step.venue.length).trimEnd() : step?.title;

  async function shareGuide() {
    const url = new URL(guideSharePath(location.hash), location.origin).href;
    try { await navigator.clipboard.writeText(url); shareCopied = true; setTimeout(() => shareCopied = false, 2500); }
    catch { shareFallback = url; }
  }
  async function selectTab(next: "guide" | "reviews") {
    tab = next;
    if (next === "reviews") playing = false;
    await tick(); window.scrollTo({ top: 0 }); heading?.focus({ preventScroll: true });
  }
  async function navigate(next: number) {
    tab = "guide";
    chapter = next;
    frame = 0;
    elapsed = 0;
    playing = !reducedMotion;
    await tick();
    window.scrollTo({ top: 0, behavior: "instant" });
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
    if (!playing && frame === (step?.frames.length ?? 0) - 1 && elapsed >= frameDuration) replay();
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
    let previousTick: number | undefined;
    const media = matchMedia("(prefers-reduced-motion: reduce)");
    const motionChanged = () => { reducedMotion = media.matches; if (reducedMotion) playing = false; };
    motionChanged(); media.addEventListener("change", motionChanged);
    const visibilityChanged = () => { documentVisible = !document.hidden; previousTick = undefined; };
    visibilityChanged(); document.addEventListener("visibilitychange", visibilityChanged);
    const keyboard = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !document.querySelector('[aria-modal="true"]')) { event.preventDefault(); onClose(); }

    };
    document.addEventListener("keydown", keyboard);
    heading?.focus({ preventScroll: true });
    let animationId: number;
    const advance = (now: number) => {
      animationId = requestAnimationFrame(advance);
      if (!playing || !documentVisible || !sceneVisible || !step || walletSwap) { previousTick = undefined; return; }
      const delta = previousTick === undefined ? 0 : Math.min(now - previousTick, 100);
      previousTick = now;
      elapsed = Math.min(frameDuration, elapsed + delta);
      if (elapsed >= frameDuration) {
        if (frame < step.frames.length - 1) { frame += 1; elapsed = 0; }
        else playing = false;
      }
    };
    animationId = requestAnimationFrame(advance);
    return () => {
      cancelAnimationFrame(animationId);
      document.removeEventListener("keydown", keyboard);
      document.removeEventListener("visibilitychange", visibilityChanged);
      media.removeEventListener("change", motionChanged);

    };
  });
</script>

<svelte:head><title>{route.source_currency} → {route.target_currency ?? route.entry_asset} · {copy("Exchange guide")} · Pay3Flow</title></svelte:head>

<div class="guidePage" role="region" aria-labelledby="route-instructions-title" tabindex="-1" data-testid="route-guide">
  <h2 id="route-instructions-title" class="srOnly">{copy("How to complete this exchange")}</h2>
  <div class="guideToolbar"><button type="button" on:click={onClose}>← {copy("Back to routes")}</button><button type="button" on:click={shareGuide} aria-live="polite">{#if shareCopied}<span aria-hidden="true">✓</span>{:else}<svg class="shareIcon" width="18" height="18" viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="M14 3h7v7m0-7-9 9M10 3H7a4 4 0 0 0-4 4v10a4 4 0 0 0 4 4h10a4 4 0 0 0 4-4v-3" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" /></svg>{/if} {copy(shareCopied ? "Link copied" : "Share guide")}</button></div>
  {#if shareFallback}<div class="shareFallback"><label>{copy("Guide link")}<input readonly value={shareFallback} on:focus={(event) => event.currentTarget.select()} /></label></div>{/if}
  <div class="guideLayout">
    <aside class="chapters">
      <span class="eyebrow">{copy("YOUR ROUTE")}</span>
      <div class="routePair"><span><img src={fiatFlagUrl(route.source_currency) ?? assetIcon(route.source_currency)} alt="" />{route.source_currency}</span><svg class="pairArrow" width="20" height="20" viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="M3 12h16m-6-6 6 6-6 6" stroke="currentColor" stroke-width="2.8" stroke-linecap="round" stroke-linejoin="round" /></svg><span><img src={fiatFlagUrl(route.target_currency ?? route.entry_asset) ?? assetIcon(route.target_currency ?? route.entry_asset)} alt="" />{route.target_currency ?? route.entry_asset}</span></div>
      <button type="button" class="overviewButton" class:current={chapter === -1 && tab === "guide"} aria-current={chapter === -1 && tab === "guide" ? "step" : undefined} on:click={() => navigate(-1)}><img class="beforeIcon" src="/icons/ui/guide-before.svg" alt="" width="24" height="24" />{copy("Before you begin")}</button>
      <ol aria-label={copy("Exchange steps")}>
        {#each steps as item, index}
          <li data-testid="instruction-step" class:complete={index < completed} class:active={chapter === index && tab === "guide"}>
            <button type="button" aria-label={copy("Step {number}: {title}", { number: index + 1, title: item.title })} disabled={index > completed} aria-current={chapter === index && tab === "guide" ? "step" : undefined} on:click={() => navigate(index)}>
              <span class="stepNumber">{index < completed ? "✓" : index + 1}</span><span class="chapterCopy"><strong>{item.title}</strong><small>{item.venue}</small></span>
            </button>
          </li>
        {/each}
      </ol>
      <div class="chapterProgress"><div><span>{copy("Your progress")}</span><strong>{completed}/{steps.length}</strong></div><div class="progressTrack"><span style={`width:${steps.length ? completed / steps.length * 100 : 0}%`}></span></div><p>{copy("Continue after completing each step on the platform.")}</p></div>
      <button type="button" class="reviewsTab" class:current={tab === "reviews"} aria-current={tab === "reviews" ? "page" : undefined} on:click={() => selectTab("reviews")}><span>☆</span>{copy("Customer reviews")}<span>↗</span></button>

    </aside>
    <div class="guideMain">
      {#if tab === "reviews"}
        <div class="reviewsPage"><span class="eyebrow">{copy("THE PEOPLE BEHIND THE RATINGS")}</span><h1 bind:this={heading} tabindex="-1">{copy("Real experiences. A clearer choice.")}</h1><p class="lead">{copy("Read customer feedback about the platforms and advertisers on your route.")}</p>{#if reviewSources.length}<div class="reviewSections">{#each reviewSources as source (source.key)}<ExternalReviews provider={source.provider} profileUrl={source.profileUrl} sourceUrl={source.sourceUrl} sourceName={source.sourceName} venueName={source.venue} iconUrl={venueIcon(source.provider ?? steps.find(item => item.venue === source.sourceName)?.provider ?? "")} />{/each}</div>{:else}<div class="reviewsEmpty"><span>“</span><p>{copy("No review sources are available for this route yet.")}</p></div>{/if}</div>
      {:else}
      {#key chapter}
        <div class="chapterEntrance">
          <div class="chapterHeading"><span class="eyebrow">{finished ? copy("GUIDE COMPLETED") : chapter === -1 ? copy("LET’S GET YOU THERE") : copy("STEP {current} OF {total}", { current: chapter + 1, total: steps.length })}</span></div>
          <h1 bind:this={heading} tabindex="-1">{finished ? copy("Every step. Done.") : headingTitle ?? copy("A clear route. At your pace.")}{#if step}{" "}{#if !titleIncludesVenue}{copy("on")}{" "}{/if}<span class="headingVenue"><img src={venueIcon(step.provider)} alt="" />{step.venue}</span>{/if}</h1>
          <p class="lead">{finished ? copy("You have confirmed every step. Check the final balance in your bank, wallet or exchange account.") : walletSwap ? copy("Review this swap and confirm it in your connected wallet.") : walletProviderStep && route.execution ? copy("Follow the instructions on {venue}, or connect your wallet to swap here.", { venue: step.venue }) : step?.summary ?? copy("First, see how your exchange works. Then follow the animated walkthrough, one step at a time.")}</p>
          {#if !walletSwap}
          <div class="chapterContent" class:spotContent={Boolean(spot)}>
            <div class="visualColumn">
              <div class="videoStage" class:spotStage={Boolean(spot)} class:paused={!playing} use:sceneVisibility>
              <InstructionScene {route} {steps} {step} {frame} {finished} progress={elapsed === 0 && !playing ? 1 : elapsed / frameDuration} playing={playing && documentVisible && sceneVisible && !reducedMotion} />
              {#if step}
                <div class="sceneOverlay">
                  <button type="button" class="sceneArrow previous" on:click={() => selectFrame(Math.max(0, frame - 1))} disabled={frame === 0} aria-label={copy("Previous scene")}>‹</button>
                  <button type="button" class="centerPlayback" on:click={togglePlayback} aria-label={copy(playing ? "Pause walkthrough" : "Play walkthrough")} data-testid="center-playback">{#if playing}<img class="pauseIcon" src="/icons/ui/guide-pause.png" alt="" width="24" height="24" />{:else}<svg class="playIcon" width="24" height="24" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M8 4.5v15L20 12 8 4.5Z" /></svg>{/if}</button>
                  <button type="button" class="sceneArrow next" on:click={() => selectFrame(Math.min(step!.frames.length - 1, frame + 1))} disabled={frame === step.frames.length - 1} aria-label={copy("Next scene")}>›</button>
                </div>
              {/if}
              </div>
              {#if step}
                <div class="playerControls">
                  <button type="button" on:click={togglePlayback} aria-label={copy(playing ? "Pause walkthrough" : "Play walkthrough")} aria-pressed={playing} data-testid="playback-toggle">{#if playing}<img class="pauseIcon" src="/icons/ui/guide-pause.png" alt="" width="24" height="24" />{:else}<svg class="playIcon" width="24" height="24" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M8 4.5v15L20 12 8 4.5Z" /></svg>{/if}</button>
                  <div class="playerTimeline" aria-label={copy("Walkthrough scenes")}>{#each step.frames as item, index}<button type="button" aria-label={copy("Scene {number}: {title}", { number: index + 1, title: item.title })} aria-pressed={index === frame} title={item.title} on:click={() => selectFrame(index)}><b>{String(index + 1).padStart(2, "0")}</b><span style={`transform:scaleX(${index < frame ? 1 : index === frame ? Math.max(.04, elapsed / frameDuration) : 0})`}></span></button>{/each}</div>
                  <span class="sceneCount">{String(frame + 1).padStart(2, "0")} / {String(step.frames.length).padStart(2, "0")}</span>
                  <button type="button" on:click={replay} aria-label={copy("Replay walkthrough")}>↻</button>
                </div>
              {/if}
            </div>
            <section class="explanation" aria-label={copy("Step instructions")}>
              {#if step}
                <span class="eyebrow">{copy("WHAT TO DO")}</span>
                <div class="frameList">{#each step.frames as item, index}<button type="button" class:selected={index === frame} aria-pressed={index === frame} on:click={() => selectFrame(index)}><span class="frameNumber">{String(index + 1).padStart(2, "0")}</span><span><strong>{item.title}</strong><span class="frameText">{item.text}</span></span>{#if index === frame}<span class="frameIndicator">←</span>{/if}</button>{/each}</div>
                {#if step.guideSteps.length}<ul class="stepNotes">{#each step.guideSteps as note}<li>{note}</li>{/each}</ul>{/if}
                {#if step.guide?.links.length}<div class="guideLinks">{#each step.guide.links as link}<a href={link.url} target="_blank" rel="noreferrer noopener">{link.label} ↗</a>{/each}</div>{/if}
                {#if spot && step.notes.length}<ul class="stepNotes">{#each step.notes as note}<li>{note}</li>{/each}</ul>{/if}
              {:else if finished}
                <span class="eyebrow">{copy("YOUR CHECKLIST")}</span>
                <div class="finishChecklist">{#each steps as item}<div><span>✓</span><p>{item.title}</p></div>{/each}</div>
                <div class="checkpoint"><span>◎</span><div><strong>{copy("Confirmed by you")}</strong><p>{copy("This checklist records your confirmations. It does not verify payments or balances.")}</p></div></div>
              {:else}
                <span class="eyebrow">{copy("THE PLAN")}</span>
                <div class="planItem"><span>01</span><div><strong>{copy("See the whole picture")}</strong><p>{copy("Your route is split into {count} steps. We explain each action before you do it.", { count: steps.length })}</p></div></div>
                <div class="planItem"><span>02</span><div><strong>{copy("Follow the demonstration")}</strong><p>{copy("Watch the highlights, pause whenever you need, and open the actual platform from the step.")}</p></div></div>
                <div class="planItem"><span>03</span><div><strong>{copy("Confirm, then continue")}</strong><p>{copy("Once the payment or balance arrives, press “Done, continue”. You decide when to move on.")}</p></div></div>
                <div class="estimate"><span>{copy("Estimated output:")}</span><strong>{tutorialMoney(route.target_amount_minor, route.target_currency, route.target_amount)}</strong><small>{copy("Check the current quote on the platform before exchanging.")}</small><button type="button" class="continueButton" disabled={!steps.length} on:click={() => navigate(0)} data-testid="start-guide">{copy("Let’s begin")} <span>→</span></button></div>
              {/if}
            </section>
          </div>
          {/if}
          {#if walletProviderStep && !walletSwap && walletProviderUrl && step}
            <a class="walletProviderLink" href={walletProviderUrl} target="_blank" rel="noreferrer noopener">{copy("Open {venue}", { venue: step.venue })} <span aria-hidden="true">↗</span></a>
          {/if}
          {#if spot && step?.url}
            <a class="walletProviderLink" data-testid="spot-market-link" href={step.url} target="_blank" rel="noreferrer noopener">{copy("Open {venue} Spot", { venue: step.venue })} · {spot.label} <span aria-hidden="true">↗</span></a>
          {/if}
          {#if step?.execution && route.execution}
            <div class="swapExecution" class:connected={walletSwap}>{#key route.route_id}<RouteExecutionPanel {route} {networkNames} />{/key}</div>
          {/if}
          <!-- Actual platform actions are paused for now.
          {#if step}
            <section class="realAction" aria-label={copy("On the actual platform")}>
              {#if step.offer}<AdvertiserCard offer={step.offer} label={step.direct ? copy("Direct exchange on {venue}", { venue: step.venue }) : `${copy(step.kind === "buy" ? "Seller" : "Buyer")} ${copy("on {venue}", { venue: step.venue })}`} serviceLink={step.serviceLink} {venueNames} {onOpenService} />
              {:else if step.serviceLink}<button type="button" class="platformLink" on:click={() => onOpenService(step!.serviceLink!)}>{copy("Open {venue}", { venue: step.venue })}<span>↗</span></button>
              {:else if step.url}<a class="platformLink" href={step.url} target="_blank" rel="noreferrer noopener">{copy("Open {venue}", { venue: step.venue })}<span>↗</span></a>{/if}
              {#if step.pair}<p class="marketInfo">Spot · {step.pair}{#if step.rate}<span>{copy("Conversion rate {rate}", { rate: step.rate })}</span>{/if}</p>{/if}
              {#if step.execution}<RouteExecutionPanel {route} />{/if}
              {#if step.notes.length}<ul class="stepNotes">{#each step.notes as note}<li>{note}</li>{/each}</ul>{/if}

            </section>
          {/if}
          -->
          {#if chapter === -1}
            <div class="routeNotice"><img class="beforeIcon" src="/icons/ui/guide-before.svg" alt="" width="24" height="24" /><div><strong>{copy("Before you begin")}</strong><p>{copy("Rates, limits, and offers can change. Check the provider, payment details, and network before sending money. Executable swaps are signed in your wallet. Other route steps are completed on the provider platform.")}</p>{#if route.source_bank_fee_percent != null}<p>{copy("Bank fees:")} {route.source_payment_method ?? route.source_currency}: {route.source_bank_fee_percent}%</p>{/if}{#if route.target_bank_fee_percent != null}<p>{copy("Bank fees:")} {route.target_payment_method ?? route.target_currency}: {route.target_bank_fee_percent}%</p>{/if}{#each route.warnings ?? [] as warning}<p>{warn(warning)}</p>{/each}</div></div>
          {/if}
        </div>
      {/key}
      {/if}
      {#if tab === "reviews" || chapter >= 0}
      <div class="guideActions"><div class="footerActions">{#if tab === "reviews"}<button type="button" class="continueButton" on:click={() => selectTab("guide")}>{copy("Return to guide")} <span>→</span></button>{:else}{#if chapter >= 0 && !finished}<button type="button" class="backButton" on:click={() => navigate(chapter - 1)}>← {copy("Back")}</button>{/if}{#if finished}<button type="button" class="backButton" on:click={() => navigate(0)}>{copy("Review steps")}</button><button type="button" class="continueButton" on:click={onClose} aria-label={copy("Back to routes")}>{copy("Back to routes")} <span aria-hidden="true">↗</span></button>{:else if step}<button type="button" class="continueButton" on:click={confirmStep} data-testid="confirm-instruction-step">{copy(chapter === steps.length - 1 ? "Done, finish" : "Done, continue")} <span>→</span></button>{/if}{/if}</div></div>
      {/if}
    </div>
  </div>
</div>

<style>
  .guidePage { position: relative; min-height: calc(100dvh - 82px); background: transparent; color: var(--color-text); animation: pageIn .35s ease; }
  .guidePage::before { content: ""; position: absolute; top: 0; bottom: 0; left: 50%; width: min(var(--layout-width), calc(100% - 2 * var(--page-gutter))); transform: translateX(-50%); border-inline: 1px solid var(--color-border); pointer-events: none; }
  .srOnly { position: absolute; width: 1px; height: 1px; padding: 0; overflow: hidden; clip: rect(0,0,0,0); white-space: nowrap; border: 0; }
  .guideLayout { display: grid; grid-template-columns: 246px minmax(0, 1fr); width: min(var(--layout-width), calc(100% - 2 * var(--page-gutter))); margin: 0 auto; padding-inline: 24px; }
  .chapters { padding: 26px 22px 48px 0; border-right: 1px solid var(--color-border); min-width: 0; }
  .eyebrow { font: 10px var(--font-mono); letter-spacing: .09em; color: var(--color-text-soft); }
  .routePair { display: flex; align-items: center; gap: 13px; margin-top: 15px; font-size: 24px; font-weight: 800; letter-spacing: -.05em; }.routePair > span { font-size: 20px; color: var(--color-text-soft); }
  .overviewButton { display: flex; align-items: center; gap: 13px; width: 100%; padding: 12px 8px; margin-top: 31px; border-radius: 8px; text-align: left; font-size: 12px; font-weight: 650; color: var(--color-text-soft); }.beforeIcon { flex: 0 0 auto; object-fit: contain; filter: brightness(0); } :global(html[data-theme="dark"]) .beforeIcon { filter: brightness(0) invert(1); }.overviewButton.current { color: var(--color-text); background: var(--color-panel-soft); }
  .chapters ol { list-style: none; display: grid; gap: 9px; margin-top: 19px; }
  .chapters li { position: relative; }.chapters li:not(:last-child)::after { content: ""; position: absolute; left: 20px; top: 44px; bottom: -13px; width: 1px; background: var(--color-border); }
  .chapters li button { display: flex; align-items: flex-start; gap: 12px; width: 100%; padding: 10px 6px; text-align: left; border-radius: 9px; }.chapters li button:disabled { cursor: default; color: var(--color-text-soft); }.chapters li.active button { background: var(--color-accent-soft); }
  .stepNumber { display: grid; place-items: center; flex: 0 0 28px; height: 28px; border: 1px solid var(--color-border-strong); border-radius: 50%; font: 11px var(--font-mono); }.active .stepNumber, .complete .stepNumber { background: var(--color-accent); border-color: var(--color-accent); color: #182414; }
  .chapterCopy { display: grid; gap: 6px; padding-top: 3px; min-width: 0; }.chapterCopy strong { font-size: 11px; font-weight: 700; line-height: 1.55; overflow-wrap: anywhere; }.chapterCopy small { font-size: 10px; color: var(--color-text-soft); }
  .chapterProgress { margin-top: 36px; padding-top: 24px; border-top: 1px solid var(--color-border); }.chapterProgress > div:first-child { display: flex; justify-content: space-between; font-size: 10px; color: var(--color-text-soft); }.chapterProgress strong { font: 11px var(--font-mono); color: var(--color-text); }.progressTrack { margin-top: 12px; height: 3px; border-radius: 3px; background: var(--color-border); overflow: hidden; }.progressTrack > span { display: block; height: 100%; background: var(--color-accent-strong); transition: width .6s ease; }.chapterProgress p { margin-top: 12px; color: var(--color-text-soft); font-size: 10px; line-height: 1.8; }
  .guideMain { min-width: 0; padding: 26px 0 48px 34px; }.chapterEntrance { animation: chapterIn .5s cubic-bezier(.22,1,.36,1) both; }.chapterHeading { display: flex; align-items: center; justify-content: space-between; gap: 15px; }
  h1 { font-size: clamp(30px, 3.3vw, 49px); line-height: 1.15; font-weight: 750; letter-spacing: -.055em; margin-top: 17px; max-width: 960px; }h1:focus { outline: none; }.lead { max-width: 710px; color: var(--color-text-soft); font-size: 13px; line-height: 1.8; margin-top: 14px; }
  .swapExecution.connected { max-width: 560px; margin: 24px auto 0; }
  .walletProviderLink { display: inline-flex; align-items: center; min-height: 44px; margin-top: 20px; padding: 0 16px; border-radius: 10px; background: var(--color-accent); color: #152016; font-size: 13px; font-weight: 750; }
  .chapterContent { display: grid; grid-template-columns: minmax(0, 1.1fr) minmax(260px, .85fr); gap: 33px; margin-top: 31px; align-items: start; }.visualColumn { min-width: 0; }.explanation { padding-top: 7px; min-width: 0; }
  .planItem { display: flex; align-items: flex-start; gap: 14px; margin-top: 25px; }.planItem > span { padding-top: 2px; color: var(--color-text-faint); font: 11px var(--font-mono); }.planItem strong { font-size: 14px; font-weight: 750; letter-spacing: -.02em; }.planItem p { color: var(--color-text-soft); font-size: 12px; line-height: 1.85; margin-top: 7px; }
  .estimate { display: grid; gap: 10px; margin-top: 31px; padding: 20px; border: 1px solid var(--color-border); border-radius: 13px; background: var(--color-paper); }.estimate > span { color: var(--color-text-soft); font-size: 11px; }.estimate strong { font-size: 27px; font-weight: 750; letter-spacing: -.04em; overflow-wrap: anywhere; }.estimate .continueButton { width: 100%; margin-top: 6px; }.estimate small { font-size: 10px; color: var(--color-text-soft); line-height: 1.7; }
  .frameList { display: grid; gap: 7px; margin-top: 15px; }.frameList button { display: flex; align-items: flex-start; width: 100%; gap: 11px; text-align: left; padding: 13px 12px; border: 1px solid transparent; border-radius: 11px; transition: background .3s, border-color .3s; }.frameList button.selected { border-color: var(--color-border); background: var(--color-paper); }.frameNumber { flex: 0 0 22px; margin-top: 2px; font: 10px var(--font-mono); color: var(--color-text-faint); }.selected .frameNumber { color: var(--color-accent-text); }.frameList strong { font-size: 12px; font-weight: 750; line-height: 1.5; }.frameText { display: block; color: var(--color-text-soft); font-size: 11px; line-height: 1.8; margin-top: 5px; }.frameIndicator { font-size: 15px; color: var(--color-accent-text); margin-left: auto; }
  .checkpoint { display: flex; gap: 10px; padding: 17px; background: var(--color-accent-soft); border: 1px solid var(--color-border); border-radius: 11px; margin-top: 19px; }.checkpoint > span { color: var(--color-accent-text); font-size: 18px; }.checkpoint strong { font-size: 11px; }.checkpoint p { font-size: 11px; line-height: 1.8; margin-top: 5px; color: var(--color-text-soft); }
  .playerControls { display: flex; align-items: center; gap: 10px; padding: 8px 4px; margin-top: 5px; }.playerControls > button { display: grid; place-items: center; font-size: 17px; color: var(--color-text-soft); width: 32px; height: 32px; }.playerTimeline { display: flex; gap: 4px; flex: 1; }.playerTimeline button { position: relative; flex: 1; min-width: 0; height: 32px; background: transparent; }.playerTimeline button::before { content: ""; position: absolute; left: 0; right: 0; height: 3px; top: 15px; border-radius: 3px; background: var(--color-border); }.playerTimeline button > span { position: absolute; top: 15px; left: 0; height: 3px; border-radius: 3px; background: var(--color-accent-text); transition: width .1s linear; }.sceneCount { white-space: nowrap; font: 9px var(--font-mono); color: var(--color-text-soft); }
  /* .realAction { padding: 22px; margin-top: 28px; border: 1px solid var(--color-border); border-radius: 16px; background: var(--color-paper); }.platformLink { display: flex; align-items: center; justify-content: space-between; gap: 18px; width: fit-content; min-height: 44px; padding: 0 16px; background: var(--color-accent); color: #152016; border-radius: 10px; font-size: 12px; font-weight: 750; }.platformLink:hover { background: #c3ff22; }.platformLink > span { font-size: 19px; }.marketInfo { margin-top: 14px; font-size: 12px; font-family: var(--font-mono); }.marketInfo > span { display: block; font-family: var(--font-sans); margin-top: 5px; color: var(--color-text-soft); } */
  .guideLinks { display: flex; flex-wrap: wrap; gap: 15px; margin-top: 13px; }.guideLinks a { text-decoration: underline; text-underline-offset: 3px; }
  .routeNotice { display: flex; align-items: flex-start; gap: 12px; margin-top: 30px; padding-top: 20px; border-top: 1px solid var(--color-border); color: var(--color-text-soft); font-size: 11px; line-height: 1.9; }.routeNotice > span { font-size: 18px; }.routeNotice strong { color: var(--color-text); font-size: 11px; }.routeNotice p { margin-top: 5px; }.finishChecklist { margin-top: 22px; display: grid; gap: 17px; }.finishChecklist > div { display: flex; gap: 11px; font-size: 12px; line-height: 1.7; }.finishChecklist > div > span { color: var(--color-accent-text); }
  .guideActions { margin-top: 32px; display: flex; justify-content: flex-end; align-items: center; padding: 20px 0; border-top: 1px solid var(--color-border); }.footerActions { display: flex; align-items: center; gap: 24px; }.backButton { font-size: 12px; color: var(--color-text-soft); }.continueButton { display: flex; align-items: center; justify-content: space-between; gap: 35px; min-height: 47px; padding: 0 21px; border-radius: 10px; background: var(--color-accent); color: #152016; font-size: 13px; font-weight: 800; }.continueButton > span { font-size: 22px; font-weight: 400; }.continueButton:hover { background: #c3ff22; }.continueButton:disabled { opacity: .5; cursor: default; }
  .guideToolbar { display: flex; justify-content: space-between; width: min(var(--layout-width), calc(100% - 2 * var(--page-gutter))); margin: 0 auto; padding: 18px 24px; border-bottom: 1px solid var(--color-border); }.guideToolbar button { min-height: 44px; font-size: 12px; color: var(--color-text-soft); display: flex; align-items: center; gap: 8px; }.guideToolbar button:hover { color: var(--color-text); }
  .shareIcon { display: block; flex: 0 0 auto; }
  .routePair { gap: 10px; font-size: 18px; flex-wrap: wrap; }.routePair > span { display: inline-flex; align-items: center; gap: 7px; color: var(--color-text); font-size: inherit; }.routePair img { width: 26px; height: 26px; border-radius: 50%; }.routePair b { color: var(--color-text-soft); font-weight: 400; }
  .headingVenue { overflow-wrap: anywhere; }.headingVenue img { display: inline-block; width: .8em; height: .8em; vertical-align: -.02em; margin-left: .16em; margin-right: .22em; border-radius: 50%; }
  .videoStage { position: relative; border-radius: 24px; }.sceneOverlay { position: absolute; inset: 0; display: flex; align-items: center; justify-content: space-between; padding: 0 12px; pointer-events: none; }.sceneOverlay button { pointer-events: auto; display: grid; place-items: center; background: #102016cf; color: #f4f8ef; border: 1px solid #f3ffe12b; box-shadow: 0 4px 25px #0003; }.sceneArrow { width: 44px; height: 54px; border-radius: 12px; font-size: 34px; }.sceneArrow:disabled { opacity: .25; cursor: default; }.centerPlayback { width: 68px; height: 68px; border-radius: 50%; font-size: 24px; opacity: 0; transition: opacity .18s, scale .18s; }.videoStage:hover .centerPlayback, .videoStage:focus-within .centerPlayback, .videoStage.paused .centerPlayback { opacity: 1; }.centerPlayback:hover { scale: 1.05; }.sceneArrow:hover:not(:disabled) { background: #b5f500; color: #172510; }
  .playerTimeline button { height: 42px; border-radius: 7px; border: 1px solid var(--color-border); background: var(--color-paper); cursor: pointer; overflow: hidden; }.playerTimeline button:hover { border-color: var(--color-accent-text); background: var(--color-accent-soft); }.playerTimeline button::before { top: auto; bottom: 0; height: 3px; }.playerTimeline button > span { top: auto; bottom: 0; width: 100%; transform-origin: left; transition: transform .1s linear; }.playerTimeline b { font: 10px var(--font-mono); color: var(--color-text-soft); }.playerTimeline button[aria-pressed="true"] { border-color: var(--color-accent-text); }.playerTimeline button[aria-pressed="true"] b { color: var(--color-text); }
  .stepNotes { padding-left: 18px; margin-top: 18px; display: grid; gap: 10px; font-size: 12px; line-height: 1.8; color: var(--color-text-soft); }
  .reviewsTab { display: flex; align-items: center; gap: 12px; width: 100%; margin-top: 18px; padding: 13px 10px; border-radius: 10px; font-size: 12px; text-align: left; border: 1px solid var(--color-border); }.reviewsTab > span:first-child { font-size: 22px; }.reviewsTab > span:last-child { margin-left: auto; color: var(--color-text-soft); }.reviewsTab.current { background: var(--color-accent-soft); border-color: var(--color-accent-text); }
  .reviewSections { display: grid; gap: 36px; margin-top: 28px; } .pairArrow { flex: 0 0 auto; color: var(--color-text-soft); } .pauseIcon { object-fit: contain; filter: brightness(0) invert(1); } .playerControls .pauseIcon { filter: none; } :global(html[data-theme="dark"]) .playerControls .pauseIcon { filter: brightness(0) invert(1); } .reviewsEmpty { padding: 50px 24px; border: 1px solid var(--color-border); border-radius: 24px; background: var(--color-paper); }.reviewsEmpty > span { font-size: 80px; color: var(--color-accent-text); }.reviewsEmpty p { color: var(--color-text-soft); }.shareFallback { padding: 18px; }.shareFallback label { display: grid; gap: 8px; }.shareFallback input { width: 100%; padding: 12px; }
  @keyframes pageIn { from { opacity: 0;  } }@keyframes chapterIn { from { opacity: 0; transform: translateY(14px); } }
  .chapterContent.spotContent { grid-template-columns: minmax(0, 1fr); gap: 20px; }
  .spotContent .frameList { grid-template-columns: repeat(5, minmax(0, 1fr)); gap: 8px; }
  .spotContent .frameList button { flex-direction: column; gap: 8px; padding: 14px; border-color: var(--color-border); border-radius: 10px; }
  .spotContent .frameList button.selected { border-color: var(--color-accent-text); }
  .spotContent .frameIndicator { display: none; }
  .spotStage .sceneOverlay { align-items: flex-end; padding: 0 12px 82px; }
  .spotStage .sceneArrow { width: 32px; height: 36px; font-size: 25px; border-radius: 8px; }
  .spotStage .centerPlayback { width: 44px; height: 44px; }
  @media (max-width: 1100px) { .spotContent .frameList { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
  @media (max-width: 760px) { .spotContent .frameList { grid-template-columns: minmax(0, 1fr); }.spotContent .frameList button { flex-direction: row; }.spotStage .sceneOverlay { padding-bottom: 84px; } }
  @media (min-width: 761px) { .guideLayout { min-height: calc(100dvh - 82px); } }
  @media (max-height: 820px) and (min-width: 901px) {.guideMain { padding-top: 25px; }h1 { font-size: 38px; margin-top: 12px; }.lead { margin-top: 10px; }.chapterContent { margin-top: 24px; } }
  @media (max-width: 1100px) { .guideLayout { grid-template-columns: 205px minmax(0,1fr); }.chapters { padding-left: 0; padding-right: 18px; }.guideMain { padding-left: 27px; padding-right: 0; }.chapterContent { gap: 20px; grid-template-columns: minmax(0,1fr) minmax(235px,.85fr); }.guideHeader { padding-left: 24px; padding-right: 24px; } }
  @media (max-width: 900px) and (min-width: 761px) { .guideLayout { grid-template-columns: 170px minmax(0,1fr); }.chapters { padding-left: 0; padding-right: 12px; }.chapterContent { grid-template-columns: minmax(0,1fr); }.guideMain { padding-left: 25px; padding-right: 0; }.frameList { grid-template-columns: 1fr 1fr; } }
  @media (max-width: 760px) {
    .guideLayout { display: block; padding-inline: 0; }.guideToolbar { padding: 8px 18px; }.reviewsTab { width: auto; flex: 0 0 auto; margin: 0; min-height: 44px; white-space: nowrap; }.reviewsTab > span:last-child { display: none; }.chapters { padding: 15px 18px; border-right: 0; border-bottom: 1px solid var(--color-border); }.chapters > .eyebrow, .routePair, .chapterCopy { display: none; }.chapters { display: grid; grid-template-columns: auto minmax(0, 1fr); align-items: center; gap: 12px; }.chapterProgress, .reviewsTab { grid-column: 1 / -1; }.chapterProgress { margin-top: 4px; padding-top: 14px; }.chapterProgress p { display: none; }.reviewsTab { width: 100%; }.overviewButton { flex: 0 0 auto; width: auto; margin: 0; padding: 7px 10px; font-size: 11px; }.chapters ol { display: flex; gap: 8px; margin: 0; overflow-x: auto; }.chapters li:not(:last-child)::after { display: none; }.chapters li button { padding: 5px; min-width: 44px; min-height: 44px; display: grid; place-items: center; }.stepNumber { width: 29px; height: 29px; }.guideMain { padding: 26px 18px 32px; }h1 { margin-top: 13px; font-size: 33px; }.lead { font-size: 12px; line-height: 1.85; margin-top: 11px; }.chapterContent { grid-template-columns: minmax(0,1fr); gap: 25px; margin-top: 23px; }.planItem { margin-top: 21px; }.estimate { margin-top: 24px; }.frameList button { padding: 12px 10px; }.frameText { font-size: 12px; }.frameList strong { font-size: 13px; }.checkpoint p { font-size: 12px; }/* .realAction { padding: 17px; margin-top: 24px; } */.routeNotice { font-size: 11px; }
    .guideActions { padding: 18px 0 max(18px, env(safe-area-inset-bottom)); gap: 12px; }.footerActions { justify-content: space-between; width: 100%; gap: 14px; }.continueButton { min-height: 48px; flex: 1; justify-content: center; gap: 23px; }.backButton { flex: 0 0 auto; font-size: 11px; }.playerControls > button { min-width: 44px; min-height: 44px; }.playerTimeline button { min-width: 0; }
  }
  @media (prefers-reduced-motion: reduce) { .guidePage, .chapterEntrance { animation: none; } }
</style>
