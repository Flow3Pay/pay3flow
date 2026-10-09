<script lang="ts">
  import { locale, t } from "$lib/i18n";
  import type { RouteCandidate } from "$lib/exchange";
  import type { TutorialStep } from "$lib/route-tutorial";
  import { ease, portion } from "../p2p/profile-motion";
  import { bestchangeExchanger } from "./frames";

  export let route: RouteCandidate;
  export let step: TutorialStep;
  export let frame = 0;
  export let playing = true;
  export let progress = 1;
  $: copy = (key: string, params: Record<string, string | number> = {}) => t(key, params, $locale);
  $: exchanger = bestchangeExchanger(route, step, copy);
  $: kind = step.frames[frame]?.kind;
  $: travel = ease(portion(progress, 0, .65));
  $: scroll = kind === "review" ? ease(portion(progress, .15, .65)) : kind === "verify" ? 1 - ease(portion(progress, 0, .35)) : 0;
  $: cursorX = kind === "open" ? 88 - 28 * travel : kind === "review" ? 60 - 10 * travel : kind === "verify" ? 50 - 37 * travel + 42 * ease(portion(progress, .68, 1)) : 55;
  $: cursorY = kind === "open" ? 25 + 22 * travel : kind === "review" ? 47 + 117 * travel : kind === "verify" ? 164 - 76 * travel - 41 * ease(portion(progress, .68, 1)) : 47;
  $: clicked = kind === "act" && progress >= .5;
</script>

<div class="exchangerCard" class:paused={!playing} data-testid="bestchange-exchanger-card" data-frame-kind={kind} data-handoff={clicked} style={`--scroll:${scroll};--cursor-x:${cursorX}cqw;--cursor-y:${cursorY}px`}>
  <div class="nav"><strong><b>BEST</b> CHANGE</strong><span>{copy("Exchangers")}</span><span>{copy("Monitoring")}</span></div>
  <div class="layout"><aside class="directions" class:highlight={kind === "verify" && progress > .3}><div class="directionTitles"><span>{copy("You send")}</span><span>{copy("You receive")}</span></div><div class="selectedDirection" data-testid="bestchange-direction"><strong>{step.from}</strong><strong>{step.to}</strong></div><div class="methods"><span>{step.kind === "buy" ? route.source_payment_method ?? step.network : step.network}</span><span>{step.kind === "sell" ? route.target_payment_method ?? step.targetNetwork : step.targetNetwork}</span></div>{#each ["BTC", "ETH", "USDT", "USDC", "AMD", "RUB", "USD"] as asset}<div class="currencyRow"><span>{asset}</span><span>{asset}</span></div>{/each}</aside>
    <div class="viewport"><div class="page">
      <div class="profile"><h3>{copy("Exchange office {exchanger}", { exchanger })}</h3><div class="website" class:highlight={kind === "act"} class:clicked data-testid="bestchange-website-link">{copy("Go to {exchanger} website", { exchanger })} ↗{#if clicked}<i></i>{/if}</div><div class="facts"><span>{copy("Status")}</span><strong>{copy("Check on the platform")}</strong><span>{copy("Reviews")}</span><strong>—</strong><span>{copy("Reserves")}</span><strong>{step.offer ? `${Number(step.offer.available_asset).toLocaleString()} ${step.offer.asset}` : "—"}</strong></div></div>
      <div class="reviews" data-testid="bestchange-reviews"><div class="reviewTabs"><span>{copy("Reviews")}</span><span>{copy("Statistics")}</span></div><small>{copy("Illustrative reviews")}</small>{#each ["Check feedback about payment speed.", "Read comments about the trading experience.", "Read any complaints and the exchanger's replies."] as text, index}<div class="reviewRow"><div><span>{index === 1 ? "M***" : "A***"}</span><i></i></div><p>{copy(text)}</p><span class="reply">{copy("Expand replies")} ▾</span></div>{/each}</div>
    </div><div class="scrollTrack"><span></span></div></div>
  </div>
  <div class="cursorTrack"><svg viewBox="0 0 32 38" fill="none"><path d="M3 2L27 22L15 24L9 35L3 2Z" fill="#333" stroke="white" stroke-width="2.5" stroke-linejoin="round" /></svg></div>
</div>

<style>
  .exchangerCard { position: relative; box-sizing: border-box; width: calc(100% - 48px); max-width: 560px; height: 310px; margin: 64px auto 100px; overflow: hidden; border: 1px solid #c5d198; border-radius: 12px; background: linear-gradient(#94b725, #f4f5ec); color: #333; font-family: Arial, sans-serif; container-type: inline-size; box-shadow: 0 18px 50px #33401118; }.nav { height: 36px; display: flex; align-items: center; gap: 15px; padding: 0 12px; box-sizing: border-box; font-size: 8px; color: #fff; }.nav > strong { font-size: 12px; color: #171717; padding: 3px; border-radius: 2px; background: white; margin-right: auto; }.nav b { color: white; background: #171717; padding: 2px; }
  .layout { display: grid; grid-template-columns: 29% minmax(0, 1fr); gap: 8px; padding: 0 9px; height: calc(100% - 36px); }.directions { align-self: start; background: #79a329; border: 3px solid white; border-radius: 9px; padding: 7px 5px; color: #fff; font-size: 8px; }.directionTitles, .currencyRow, .selectedDirection, .methods { display: grid; grid-template-columns: 1fr 1fr; gap: 5px; }.directionTitles { font-size: 8px; padding-bottom: 7px; }.selectedDirection { background: #496d13; border-radius: 3px; padding: 6px 3px; font-size: 10px; }.methods { font-size: 6px; line-height: 1.4; padding: 5px 3px; overflow-wrap: anywhere; }.currencyRow { padding: 6px 3px; }.currencyRow:nth-child(even) { background: #ffffff16; }
  .viewport { position: relative; overflow: hidden; border-radius: 8px 8px 0 0; background: #f1f1eb; }.page { padding: 9px; transform: translate3d(0, calc(-140px * var(--scroll)), 0); will-change: transform; }.profile { height: 134px; box-sizing: border-box; padding: 8px; border-radius: 7px; background: #fff; }.profile h3 { height: 24px; line-height: 12px; overflow: hidden; margin: 0 0 7px; font-size: 11px; font-weight: 400; overflow-wrap: anywhere; }.website { position: relative; width: fit-content; font-size: 9px; color: #008fce; border-bottom: 1px dashed #008fce; padding: 2px 0; }.website i { position: absolute; top: -3px; left: 55%; width: 16px; height: 16px; border: 2px solid #008fce66; border-radius: 50%; }.clicked { background: #e6f5ff; }.facts { display: grid; grid-template-columns: .7fr 1fr; margin-top: 12px; gap: 7px; font-size: 8px; }.facts strong { font-weight: 400; font-size: 7px; overflow-wrap: anywhere; }.reviews { margin-top: 12px; background: #e4e4dc; border-radius: 7px; padding: 7px; }.reviewTabs { display: flex; gap: 6px; font-size: 9px; }.reviewTabs > span { padding: 3px 8px; border-radius: 12px; background: linear-gradient(#fafafa, #ccc); }.reviewTabs > span:first-child { color: white; background: linear-gradient(#666, #222); }.reviews > small { display: block; font-size: 7px; color: #888; margin: 9px 0; }.reviewRow { padding: 9px; background: #f4f4ee; border: 1px solid #ccc; border-radius: 5px; margin-bottom: 8px; }.reviewRow > div { display: flex; justify-content: space-between; font-size: 8px; }.reviewRow i { height: 4px; width: 35px; background: #d6d8cc; border-radius: 2px; }.reviewRow p { margin: 9px 0; font-size: 9px; line-height: 1.4; }.reply { color: #008fce; font-size: 7px; border-bottom: 1px dashed #008fce; }.highlight { box-shadow: 0 0 0 3px #4f821f44; }.scrollTrack { position: absolute; right: 2px; top: 5px; bottom: 5px; width: 3px; background: #e2e3da; border-radius: 3px; }.scrollTrack > span { position: absolute; top: calc(var(--scroll) * (100% - 60px)); width: 3px; height: 60px; background: #adb695; border-radius: 3px; }
  .cursorTrack { position: absolute; z-index: 2; top: 36px; left: 0; width: 22px; transform: translate3d(var(--cursor-x), var(--cursor-y), 0); will-change: transform; pointer-events: none; }.cursorTrack svg { display: block; width: 22px; height: 27px; filter: drop-shadow(0 2px 2px #0004); }
  @container (max-width: 360px) { .nav { font-size: 7px; gap: 9px; }.nav > strong { font-size: 10px; }.directions { padding: 6px 3px; }.directionTitles { font-size: 6px; }.selectedDirection { font-size: 8px; }.methods { font-size: 5px; }.currencyRow { font-size: 7px; }.layout { gap: 5px; padding: 0 6px; }.page { padding: 6px; }.profile { padding: 6px; }.profile h3 { font-size: 10px; }.website { font-size: 8px; }.reviewRow { padding: 7px; }.reviewRow p { font-size: 8px; }.reviewTabs { font-size: 8px; } }
  @media (max-height: 820px) and (min-width: 761px) { .exchangerCard { margin-top: 28px; height: 250px; } }
  @media (max-width: 760px) { .exchangerCard { width: calc(100% - 36px); height: 246px; margin-top: 24px; } }
</style>
