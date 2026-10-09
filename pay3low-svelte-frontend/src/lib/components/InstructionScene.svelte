<script lang="ts">
  import { fiatFlagUrl } from "$lib/currency-flags";
  import { assetIcon, venueIcon } from "$lib/icons";
  import { locale, t } from "$lib/i18n";
  import type { RouteCandidate } from "$lib/exchange";
  import type { TutorialStep } from "$lib/route-tutorial";
  import { guideP2pCards, guideSwapCards } from "$lib/guides";
  export let route: RouteCandidate;
  export let steps: TutorialStep[];
  export let step: TutorialStep | undefined = undefined;
  export let frame = 0;
  export let playing = true;
  export let finished = false;
  export let progress = 1;
  $: copy = (key: string, params: Record<string, string | number> = {}) => t(key, params, $locale);
  $: swapCard = step && step.kind !== "transfer" ? guideSwapCards[step.provider.toLowerCase()] : undefined;
  $: p2pCard = step?.offer && !step.direct && (step.kind === "buy" || step.kind === "sell") ? guideP2pCards[step.provider.toLowerCase()] : undefined;
</script>

<div class="scene" class:paused={!playing} class:overview={!step} class:finished aria-hidden="true" data-testid="instruction-scene">
  <div class="sceneGrid"></div>
  {#if !step}
    <div class="orbit orbitOne"></div><div class="orbit orbitTwo"></div>
    <div class="journey">
      <div class="journeyCoin sourceCoin"><img src={fiatFlagUrl(route.source_currency) ?? assetIcon(route.source_currency)} class:fiatFlag={Boolean(fiatFlagUrl(route.source_currency))} alt="" /><span>{route.source_currency}</span></div>
      <div class="journeyLine"><span></span><span></span><span></span></div>
      <div class="journeyCoin targetCoin"><img src={fiatFlagUrl(route.target_currency ?? route.entry_asset) ?? assetIcon(route.target_currency ?? route.entry_asset)} class:fiatFlag={Boolean(fiatFlagUrl(route.target_currency ?? route.entry_asset))} alt="" /><span>{route.target_currency ?? route.entry_asset}</span></div>
    </div>
    <div class="journeyCard">
      <div class="journeyCardTop"><img class="smallLogo" src="/icons/assets/pay3flow_logo.svg" alt="" width="23" height="23" /><span>{copy(finished ? "All steps confirmed" : "One route. One step at a time.")}</span><span class="cardCheck">{finished ? "✓" : "↗"}</span></div>
      <div class="journeyChapters">{#each steps as item, index}<div style={`--order:${index}`}><span class="miniNumber">{finished ? "✓" : String(index + 1).padStart(2, "0")}</span><img src={venueIcon(item.provider)} alt="" /><span>{item.venue}</span><b>{item.kind === "transfer" ? "↗" : "⇄"}</b></div>{/each}</div>
      <div class="journeyCardBottom"><span>{copy("You control every step")}</span></div>
    </div>
    <div class="floatTag"><span>✓</span> {copy(finished ? "Confirmed by you" : "At your own pace")}</div>
  {:else}
    {#if p2pCard}
      <svelte:component this={p2pCard} {route} {step} {frame} {playing} {progress} />
    {:else if swapCard}
      <svelte:component this={swapCard} {route} {step} {frame} {playing} />
    {:else}
    <div class="browser" class:transfer={step.kind === "transfer"}>
      <div class="browserChrome"><div><i></i><i></i><i></i></div><span>{step.venue}</span><span>↗</span></div>
      <div class="browserContent">
        <div class="platform"><img src={venueIcon(step.provider)} alt="" /><strong>{step.venue}</strong><span>{copy(step.kind === "transfer" ? "Deposit" : step.offer && !step.direct ? "P2P exchange" : "Exchange")}</span></div>
        <div class="offerIdentity" class:highlight={frame === 0}>
          <span class="identityIcon">{step.kind === "transfer" ? "↗" : "⇄"}</span>
          <div><small>{copy(step.offer && !step.direct ? "Advertiser" : "Selected route")}</small><strong>{step.offer && !step.direct ? step.offer.advertiser.nickname : step.pair ?? `${step.from} → ${step.to}`}</strong>{#if step.offer && !step.direct}<small>ID {step.offer.ad_id}</small>{/if}</div><span class="verified">✓</span>
        </div>
        <div class="assetFields" class:highlight={frame === 1}>
          <div><small>{copy("You send")}</small><strong>{step.amount ?? step.from}</strong><img src={fiatFlagUrl(step.from) ?? assetIcon(step.from)} class:fiatFlag={Boolean(fiatFlagUrl(step.from))} alt="" /></div>
          <span class="fieldArrow">↓</span>
          <div><small>{copy(step.kind === "transfer" ? "Deposit address" : "You receive")}</small><strong>{step.kind === "transfer" ? "•••• •••• ••••" : step.output ?? step.to}</strong>{#if step.kind === "transfer"}<span class="copyAddress">⧉</span>{:else}<img src={fiatFlagUrl(step.to) ?? assetIcon(step.to)} class:fiatFlag={Boolean(fiatFlagUrl(step.to))} alt="" />{/if}</div>
        </div>
        <div class="networkLine" class:highlight={frame === 1 && step.kind === "transfer"}><span>{copy(step.kind === "transfer" || step.network ? "Network" : "Payment methods")}</span><strong>{step.network ?? step.offer?.payment_methods.join(", ") ?? copy("Check on the platform")}</strong></div>
        <div class="demoAction" class:highlight={frame === 2} class:received={frame === 3}>{frame === 3 ? "✓" : "↗"} {copy(frame === 3 ? "Check your actual balance" : step.kind === "transfer" ? "Confirm the transfer" : step.kind === "sell" && !step.direct ? "Wait for the payment" : "Confirm on the platform")}</div>
      </div>
      <svg class="demoCursor" class:cursorVerify={frame === 1} class:cursorAct={frame === 2} class:cursorReceive={frame === 3} viewBox="0 0 32 38" fill="none"><path d="M3 2L27 22L15 24L9 35L3 2Z" fill="#b5f500" stroke="#132015" stroke-width="2.5" /></svg>
    </div>
    {/if}
    {#if step.offer && !step.direct && !p2pCard && frame >= 2}
      <div class="paymentPreview">
        <div class="phoneSpeaker"></div><span class="paymentIcon">{step.kind === "sell" ? "↓" : "↗"}</span>
        <small>{copy(step.kind === "sell" ? "Verify incoming payment" : "Payment details in order")}</small>
        <strong>{step.kind === "sell" ? route.target_payment_method ?? step.offer.payment_methods[0] : route.source_payment_method ?? step.offer.payment_methods[0]}</strong>
        <span class="phoneAmount">{step.kind === "sell" ? step.output ?? step.to : step.amount ?? step.from}</span>
        <div class="phoneNote">{copy("Check in your bank")}</div>
      </div>
    {/if}
    {#key frame}<div class="sceneAnnotation"><span>{String(frame + 1).padStart(2, "0")}</span><strong>{step.frames[frame]?.title}</strong></div>{/key}
    <div class="sceneFootnote">{copy("Illustration · the platform interface may differ")}</div>
  {/if}
</div>

<style>
  .scene { --scene-background: #f2f4ee; --scene-grid: #4b643c14; --scene-orbit: #80a94e24; --scene-shadow: #1f31151f; --scene-card: #ffffff; --scene-tag: #ffffff; --scene-halo: #ffffff90; contain: layout paint; position: relative; min-height: 490px; height: 100%; overflow: hidden; border-radius: 24px; color: var(--color-text); background: var(--scene-background); box-shadow: inset 0 0 0 1px var(--color-border); isolation: isolate; }
  :global(html[data-theme="dark"]) .scene { --scene-background: #19211d; --scene-grid: #c9ddbd0d; --scene-orbit: #b5f50012; --scene-shadow: #0005; --scene-card: #f4f6ed; --scene-tag: #273222; --scene-halo: #ffffff04; }
  .sceneGrid { position: absolute; inset: 0; z-index: -1; background-image: linear-gradient(var(--scene-grid) 1px, transparent 1px), linear-gradient(90deg, var(--scene-grid) 1px, transparent 1px), radial-gradient(ellipse at 50% 50%, #b5f50013, transparent 65%); background-size: 32px 32px, 32px 32px, auto; }
  .browser { width: 330px; position: relative; left: calc(50% - 165px); margin: 76px 0 66px; border: 1px solid #14201620; border-radius: 15px; background: #f8faf6; color: #152016; box-shadow: 0 24px 70px var(--scene-shadow); transform: perspective(1200px) rotateY(-7deg) rotateX(3deg); animation: browserIn .8s cubic-bezier(.22,1,.36,1) both; }
  .browserChrome { display: flex; align-items: center; justify-content: space-between; padding: 10px 14px; border-bottom: 1px solid #14201615; color: #4e5b4d; font: 9px var(--font-mono); }
  .browserChrome > div { display: flex; gap: 4px; }
  .browserChrome i { width: 5px; height: 5px; border-radius: 50%; background: #c2cac0; }
  .browserContent { padding: 20px; }
  .platform { display: flex; align-items: center; gap: 7px; margin-bottom: 18px; font-size: 12px; }
  .platform img { width: 19px; height: 19px; border-radius: 50%; }
  .platform > span { margin-left: auto; font-size: 9px; color: #4e5b4d; }
  .offerIdentity { display: flex; align-items: center; gap: 10px; padding: 9px; border: 1px solid transparent; border-radius: 10px; transition: .5s ease; }
  .identityIcon { display: grid; place-items: center; flex: 0 0 32px; height: 32px; border-radius: 50%; background: #e6eadf; font-size: 20px; }
  .offerIdentity > div { display: grid; gap: 3px; min-width: 0; }
  .offerIdentity strong { overflow-wrap: anywhere; font-size: 12px; }
  small { font-size: 9px; color: #4a5746; }
  .verified { margin-left: auto; font-size: 10px; color: #426117; }
  .assetFields { display: grid; gap: 5px; margin-top: 12px; padding: 7px; border: 1px solid transparent; border-radius: 12px; transition: .5s ease; }
  .assetFields > div { display: grid; grid-template-columns: 1fr auto; gap: 5px; position: relative; padding: 11px; background: #edf0e9; border-radius: 8px; }
  .assetFields small { grid-column: 1; }
  .assetFields strong { font-size: 16px; font-weight: 700; max-width: 220px; overflow-wrap: anywhere; }
  .assetFields img { width: 23px; height: 23px; grid-column: 2; grid-row: 1 / 3; align-self: center; }
  .copyAddress { font-size: 23px; grid-column: 2; grid-row: 1 / 3; align-self: center; }
  .fieldArrow { position: absolute; z-index: 1; left: calc(50% - 11px); margin-top: 58px; width: 22px; height: 22px; display: grid; place-items: center; border: 3px solid #f8faf6; border-radius: 50%; background: #dfe5d8; font-size: 11px; }
  .networkLine { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 10px 7px; margin: 4px 0 8px; font-size: 9px; border: 1px solid transparent; border-radius: 8px; transition: .5s ease; }
  .networkLine > span { color: #4a5746; }
  .networkLine strong { max-width: 180px; text-align: right; overflow-wrap: anywhere; }
  .demoAction { display: flex; justify-content: center; align-items: center; gap: 6px; min-height: 36px; padding: 7px; border: 1px solid transparent; border-radius: 8px; background: #dce3d4; font-size: 10px; font-weight: 750; transition: .5s ease; }
  .highlight { border-color: #709c09; box-shadow: 0 0 0 4px #b5f50024; }
  .demoAction.highlight { background: #b5f500; }
  .demoAction.received { background: #193522; color: #d4f3a9; }
  .demoCursor { position: absolute; top: 0; left: 0; transform: translate3d(230px, 118px, 0); will-change: transform; width: 27px; height: 32px; filter: drop-shadow(0 3px 2px #0004); transition: transform 1s cubic-bezier(.22,1,.36,1); animation: cursorPulse 3s ease infinite; }
  .demoCursor.cursorVerify { transform: translate3d(200px, 220px, 0); }.demoCursor.cursorAct { transform: translate3d(225px, 354px, 0); }.demoCursor.cursorReceive { transform: translate3d(65px, 355px, 0); }
  .sceneAnnotation { position: absolute; bottom: 54px; right: 24px; display: flex; align-items: center; gap: 10px; max-width: calc(100% - 48px); padding: 12px 16px 12px 10px; background: #b5f500; color: #182414; border-radius: 10px; box-shadow: 0 8px 32px var(--scene-shadow); animation: annotationIn .55s cubic-bezier(.22,1,.36,1) both; }
  .sceneAnnotation > span { display: grid; place-items: center; width: 25px; height: 25px; background: #17251012; border-radius: 50%; font: 10px var(--font-mono); }
  .sceneAnnotation strong { font-size: 11px; }
  .sceneFootnote { background: var(--scene-background); padding: 4px 6px; border-radius: 4px; width: fit-content; margin: 0 auto; position: absolute; inset: auto 20px 20px; color: var(--color-text-soft); font-size: 10px; text-align: center; }
  .paymentPreview { position: absolute; left: 19px; bottom: 93px; width: 149px; display: grid; justify-items: center; gap: 8px; padding: 13px 12px; border: 3px solid #ccd4c5; border-radius: 19px; background: #f4f6ed; color: #182414; box-shadow: 0 12px 40px var(--scene-shadow); transform: rotate(-6deg); animation: phoneIn .65s cubic-bezier(.22,1,.36,1) both; }
  .phoneSpeaker { width: 29px; height: 3px; border-radius: 3px; background: #b1bca5; }.paymentIcon { display: grid; place-items: center; width: 29px; height: 29px; margin-top: 3px; border-radius: 50%; background: #b5f500; font-size: 20px; }.paymentPreview small { text-align: center; font-size: 8px; line-height: 1.5; }.paymentPreview strong { text-align: center; font-size: 10px; max-width: 100%; overflow-wrap: anywhere; }.phoneAmount { font-size: 13px; font-weight: 800; text-align: center; overflow-wrap: anywhere; }.phoneNote { margin-top: 4px; padding-top: 9px; border-top: 1px solid #18241422; font-size: 8px; color: #4a5746; }
  .orbit { position: absolute; border: 1px solid var(--scene-orbit); border-radius: 50%; width: 390px; height: 390px; top: 45px; left: calc(50% - 195px); }.orbitTwo { width: 520px; height: 520px; top: -20px; left: calc(50% - 260px); }
  .journey { display: flex; align-items: center; justify-content: center; gap: 24px; margin: 94px 0 0; }
  .journeyCoin { display: grid; place-items: center; gap: 9px; }.journeyCoin img { width: 56px; height: 56px; border-radius: 50%; box-shadow: 0 0 0 10px var(--scene-halo); }.journeyCoin > span { font: 11px var(--font-mono); }
  img.fiatFlag { object-fit: cover; border-radius: 50%; }
  .sourceCoin { animation: float 6s ease-in-out infinite; }.targetCoin { animation: float 6s ease-in-out -3s infinite; }
  .journeyLine { display: flex; gap: 10px; margin-top: -20px; }.journeyLine span { width: 5px; height: 5px; border-radius: 50%; background: var(--color-accent-text); animation: signal 2.8s ease-in-out infinite; }.journeyLine span:nth-child(2) { animation-delay: .35s; }.journeyLine span:nth-child(3) { animation-delay: .7s; }
  .journeyCard { position: relative; width: 318px; margin: 29px auto 68px; padding: 17px; background: var(--scene-card); color: #182414; border-radius: 14px; box-shadow: 0 18px 60px var(--scene-shadow); transform: rotate(-4deg); animation: cardIn 1s cubic-bezier(.22,1,.36,1) both; }
  .journeyCardTop { display: flex; align-items: center; gap: 8px; font-size: 11px; font-weight: 800; padding-bottom: 14px; }.smallLogo { display: block; flex: 0 0 23px; width: 23px; height: 23px; border-radius: 7px; object-fit: contain; }.cardCheck { margin-left: auto; }
  .journeyChapters { display: grid; gap: 6px; max-height: 200px; overflow: auto; }.journeyChapters > div { display: flex; align-items: center; gap: 9px; padding: 9px; background: #e9eddf; border-radius: 7px; font-size: 11px; animation: rowIn .65s calc(var(--order) * .1s + .3s) both; }.miniNumber { font: 9px var(--font-mono); color: #5d6d4b; }.journeyChapters img { width: 18px; height: 18px; border-radius: 50%; }.journeyChapters b { margin-left: auto; font-size: 17px; font-weight: 400; }
  .journeyCardBottom { display: flex; justify-content: space-between; margin-top: 13px; font-size: 9px; color: #60704f; }
  .floatTag { position: absolute; bottom: 46px; right: 35px; display: flex; align-items: center; gap: 8px; padding: 12px 15px; border: 1px solid var(--color-border); border-radius: 9px; background: var(--scene-tag); font-size: 11px; transform: rotate(3deg); animation: float 6s ease-in-out -2s infinite; }.floatTag > span { color: var(--color-accent-text); }
  .finished .journeyCard { transform: rotate(0); }.paused .demoCursor, .paused .sourceCoin, .paused .targetCoin, .paused .journeyLine span, .paused .floatTag { animation-play-state: paused !important; }
  @keyframes browserIn { from { opacity: 0; transform: perspective(1200px) rotateY(-12deg) rotateX(5deg) translateY(25px); } }
  @keyframes annotationIn { from { opacity: 0; transform: translateX(20px); } }
  @keyframes phoneIn { from { opacity: 0; transform: translateY(20px) rotate(-10deg); } }
  @keyframes cardIn { from { opacity: 0; transform: rotate(-9deg) translateY(30px); } }
  @keyframes rowIn { from { opacity: 0; transform: translateX(-12px); } }
  @keyframes float { 50% { translate: 0 -8px; } }
  @keyframes signal { 0%,100% { opacity: .2; transform: translateX(-3px); } 50% { opacity: 1; transform: translateX(3px); } }
  @keyframes cursorPulse { 0%,100% { scale: 1; } 50% { scale: .9; } }
  @media (max-height: 820px) and (min-width: 761px) {
    .scene { min-height: 360px; height: 360px; }.browser { margin-top: 50px; scale: .7; transform-origin: top center; }.sceneAnnotation { bottom: 38px; }.sceneFootnote { bottom: 12px; }.paymentPreview { bottom: 65px; scale: .8; transform-origin: bottom left; }
    .journey { margin-top: 65px; }.journeyCoin img { width: 39px; height: 39px; }.journeyCard { margin-top: 18px; padding: 12px; }.journeyCardTop { padding-bottom: 8px; }.journeyChapters { max-height: 113px; gap: 4px; }.journeyChapters > div { padding: 7px; }.floatTag { bottom: 16px; padding: 8px 11px; }
  }
  @media (max-width: 760px) {
    .scene { min-height: 340px; height: 340px; border-radius: 18px; }.browser { margin-top: 52px; scale: .65; transform-origin: top center; }.sceneAnnotation { bottom: 32px; right: 18px; padding: 8px 12px 8px 8px; }.sceneFootnote { bottom: 12px; font-size: 8px; }.paymentPreview { left: 10px; bottom: 54px; scale: .75; transform-origin: bottom left; }
    .journey { margin-top: 65px; gap: 20px; }.journeyCoin img { width: 39px; height: 39px; }.journeyCard { width: 260px; margin-top: 18px; padding: 12px; }.journeyCardTop { font-size: 10px; padding-bottom: 8px; }.journeyChapters { max-height: 113px; gap: 4px; }.journeyChapters > div { padding: 7px; font-size: 10px; }.journeyCardBottom { font-size: 8px; }.floatTag { bottom: 16px; right: 20px; padding: 8px 11px; font-size: 9px; }
  }
  @media (prefers-reduced-motion: reduce) { *, *::before { animation: none !important; transition: none !important; } }
</style>
