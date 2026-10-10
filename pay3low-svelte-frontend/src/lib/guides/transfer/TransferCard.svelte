<script lang="ts">
  import { assetIcon, venueIcon } from "$lib/icons";
  import { locale, t } from "$lib/i18n";
  import type { RouteCandidate } from "$lib/exchange";
  import type { TutorialStep } from "$lib/route-tutorial";

  export let route: RouteCandidate;
  export let step: TutorialStep;
  export let frame = 0;
  export let playing = true;
  export let progress = 1;

  $: copy = (key: string) => t(key, {}, $locale);
  $: sourceProvider = route.legs.find(leg => leg.kind === "entry")?.provider ?? "generic";
  $: source = step.transfer?.from ?? { provider: sourceProvider, venue: sourceProvider };
  $: destination = step.transfer?.to ?? { provider: step.provider, venue: step.venue };
  $: kind = step.frames[frame]?.kind ?? "open";
  $: moving = kind === "act" || kind === "receive";
  $: position = kind === "receive" ? 1 : kind === "act" ? Math.max(0, Math.min(1, progress)) : 0;
</script>

<div class="transferCard" class:paused={!playing} data-testid="transfer-animation" data-phase={kind}>
  <div class="assetLabel"><img src={assetIcon(step.from)} alt="" /><strong>{step.from}</strong><span>{copy("Transfer")}</span></div>
  <div class="transferRoute">
    <div class="endpoint source" class:active={kind === "verify" || kind === "act"} data-testid="transfer-source">
      <span class="endpointLabel">{copy("From")}</span>
      <div class="venueMark"><img src={venueIcon(source.provider)} alt="" /></div>
      <strong>{source.venue}</strong>
    </div>
    <div class="connection" class:moving>
      <svg class="transferArrow" viewBox="0 0 200 40" fill="none" preserveAspectRatio="none" aria-hidden="true"><path class="arrowTrack" d="M2 20H190M178 8l12 12-12 12" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" /></svg>
      <div class="token" class:visible={moving} style={`left:${8 + position * 64}%`} data-testid="transfer-token"><img src={assetIcon(step.from)} alt="" /></div>
    </div>
    <div class="endpoint destination" class:active={kind === "open" || kind === "verify" || kind === "receive"} data-testid="transfer-destination">
      <span class="endpointLabel">{copy("To")}</span>
      <div class="venueMark"><img src={venueIcon(destination.provider)} alt="" />{#if kind === "receive"}<span class="depositSignal">↓</span>{/if}</div>
      <strong>{destination.venue}</strong>
    </div>
  </div>
  <div class="network" class:active={kind === "verify"}><span>{copy("Network")}</span><strong>{step.network ?? copy("Check on the platform")}</strong></div>
</div>

<style>
  .transferCard { position: absolute; inset: 42px 52px 116px; display: flex; flex-direction: column; justify-content: center; gap: 32px; }
  .assetLabel { display: flex; align-items: center; justify-content: center; gap: 8px; font-size: 13px; }
  .assetLabel img { width: 22px; height: 22px; border-radius: 50%; }.assetLabel > span { color: var(--color-text-soft); padding-left: 8px; border-left: 1px solid var(--color-border); }
  .transferRoute { display: grid; grid-template-columns: minmax(0, 1fr) minmax(52px, 1.2fr) minmax(0, 1fr); gap: 10px; align-items: start; }
  .endpoint { display: grid; justify-items: center; gap: 14px; min-width: 0; text-align: center; }
  .endpointLabel { color: var(--color-text-soft); font: 10px var(--font-mono); text-transform: uppercase; letter-spacing: .1em; }
  .venueMark { position: relative; display: grid; place-items: center; width: 82px; height: 82px; border: 1px solid var(--color-border); border-radius: 24px; background: var(--scene-tag); box-shadow: 0 12px 32px var(--scene-shadow); transition: border-color .3s, box-shadow .3s; }
  .venueMark > img { width: 48px; height: 48px; object-fit: contain; border-radius: 12px; }
  .endpoint strong { font-size: 15px; line-height: 1.4; max-width: 100%; overflow-wrap: anywhere; }
  .endpoint.active .venueMark { border-color: var(--color-accent-text); box-shadow: 0 0 0 5px #b5f50020, 0 12px 32px var(--scene-shadow); }
  .connection { position: relative; height: 82px; margin-top: 26px; display: flex; align-items: center; color: var(--color-text-soft); }
  .transferArrow { width: 100%; height: 32px; overflow: visible; }.moving { color: var(--color-accent-text); }
  .token { position: absolute; top: 50%; transform: translate(-50%, -50%); width: 30px; height: 30px; padding: 3px; display: grid; place-items: center; border-radius: 50%; background: var(--scene-background); box-shadow: 0 0 0 5px #b5f50018; opacity: 0; }
  .token.visible { opacity: 1; }.token img { width: 24px; height: 24px; border-radius: 50%; }
  .depositSignal { position: absolute; right: -5px; bottom: -5px; display: grid; place-items: center; width: 24px; height: 24px; border-radius: 50%; background: #b5f500; color: #182414; font-size: 16px; animation: depositPulse 1.8s ease-in-out infinite; }
  .network { display: flex; flex-wrap: wrap; align-items: center; justify-content: center; gap: 6px 12px; align-self: center; max-width: 100%; padding: 10px 15px; border: 1px solid var(--color-border); border-radius: 12px; text-align: center; font-size: 11px; transition: border-color .3s, box-shadow .3s; }
  .network > span { color: var(--color-text-soft); }.network strong { overflow-wrap: anywhere; }.network.active { border-color: var(--color-accent-text); box-shadow: 0 0 0 4px #b5f50018; }
  .paused .depositSignal { animation-play-state: paused; }
  @keyframes depositPulse { 50% { box-shadow: 0 0 0 6px #b5f50026; } }
  @media (max-height: 820px) and (min-width: 761px), (max-width: 760px) {
    .transferCard { inset: 24px 52px 90px; gap: 20px; }.venueMark { width: 58px; height: 58px; border-radius: 17px; }.venueMark > img { width: 36px; height: 36px; border-radius: 9px; }.endpoint { gap: 9px; }.endpoint strong { font-size: 12px; }.connection { height: 58px; margin-top: 21px; }.transferRoute { gap: 6px; }.assetLabel { font-size: 11px; }.network { font-size: 10px; padding: 8px 10px; }
  }
  @media (prefers-reduced-motion: reduce) { .depositSignal { animation: none; }.venueMark, .network { transition: none; } }
</style>
