<script lang="ts">
  import { assetIcon } from "$lib/icons";
  import { locale, t } from "$lib/i18n";
  import type { RouteCandidate } from "$lib/exchange";
  import type { TutorialStep } from "$lib/route-tutorial";

  export let route: RouteCandidate;
  export let step: TutorialStep;
  export let frame = 0;
  export let playing = true;
  export let progress = 1;

  $: copy = (key: string, params: Record<string, string | number> = {}) => t(key, params, $locale);
  $: offer = step.offer;
  $: buying = step.kind === "buy";
  $: action = copy(buying ? "Buy {asset}" : "Sell {asset}", { asset: offer?.asset ?? step.from });
  $: frameKind = step.frames[frame]?.kind;
  $: reviews = frameKind === "review" ? progress >= .18 : frameKind === "verify" && progress < .22;
  $: numberLocale = $locale === "ru" ? "ru-RU" : $locale === "hy" ? "hy-AM" : "en-US";
  const portion = (value: number, start: number, end: number) => Math.max(0, Math.min(1, (value - start) / (end - start)));
  const ease = (value: number) => value * value * (3 - 2 * value);
  const horizontal = (percent: number, pixels = 0) => `calc(${percent}cqw + ${pixels}px)`;
  function number(value?: string | number | null, decimals = 2) {
    if (value == null || value === "") return "—";
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed.toLocaleString(numberLocale, { maximumFractionDigits: decimals }) : "—";
  }
  // Use the player's clock so pause, replay, seeking and offscreen suspension stay in sync.
  $: scroll = frameKind === "open" ? ease(portion(progress, .48, .9)) : 1;
  $: approach = ease(portion(progress, 0, .45));
  $: reviewTravel = ease(portion(progress, .35, .8));
  $: adsApproach = ease(portion(progress, 0, .22));
  $: tradeApproach = ease(portion(progress, .35, .85));
  $: cursorX = frameKind === "open" ? horizontal(87 * (1 - approach), 80 * approach)
    : frameKind === "review" ? horizontal(40 * reviewTravel, 80 * (1 - reviewTravel))
    : frameKind === "verify" ? horizontal(40 * (1 - adsApproach) + 88 * tradeApproach, 20 * (adsApproach - tradeApproach)) : "88cqw";
  $: cursorY = frameKind === "open" ? 30 + 120 * approach - 132 * scroll
    : frameKind === "review" ? 18 + 124 * ease(portion(progress, .35, .8))
    : frameKind === "verify" ? 142 - 124 * ease(portion(progress, 0, .22)) + 88 * ease(portion(progress, .35, .7)) : 106;
  $: tabClick = frameKind === "review" ? progress >= .18 && progress < .34 : frameKind === "verify" && progress >= .22 && progress < .38;
  $: clicked = frameKind === "act" && progress >= .5;
  $: requested = buying ? route.source_payment_method : route.target_payment_method;
  $: selectedPayment = offer?.payment_methods.find(method => method === requested) ?? offer?.payment_methods[0] ?? copy("Check on the platform");
</script>

<!-- Simplified advertiser profile: Ads / Reviews, followed by the advertiser's ads.
     Review placeholders are illustrative, not live feedback. -->
<div class="profileCard" class:paused={!playing} data-testid="bybit-profile-card" data-frame={frame} data-frame-kind={frameKind} data-side={buying ? "buy" : "sell"} data-tab={reviews ? "review" : "ads"} style={`--scroll:${scroll};--cursor-x:${cursorX};--cursor-y:${cursorY}px`}>
  <div class="bybitNav"><span class="wordmark">BYB<span>I</span>T</span><span>P2P</span><svg class="profileIcon" viewBox="0 0 24 24" fill="none"><circle cx="12" cy="8" r="3.5" stroke="currentColor" stroke-width="1.7" /><path d="M5 21v-2a7 7 0 0 1 14 0v2" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" /></svg></div>
  <div class="profileViewport">
    <div class="profilePage">
      <div class="profileHeader">
        <div class="identity">
          <div class="avatar">{offer?.advertiser.nickname.slice(0, 1).toUpperCase() ?? "B"}</div>
          <div><strong data-testid="bybit-advertiser">{offer?.advertiser.nickname}</strong><span class="identityMeta">{#if offer?.advertiser.is_verified}<span class="verified">✓ {copy("Verified")}</span>{/if}<span>{copy("Advertiser")}</span></span></div>
          <span class="profileArrow">↗</span>
        </div>
        <div class="statistics">
          <div><strong>{number(offer?.advertiser.completed_orders_30d, 0)}</strong><small>{copy("30-day orders")}</small></div>
          <div><strong>{offer?.advertiser.completion_rate_30d == null ? "—" : `${number(offer.advertiser.completion_rate_30d * 100)}%`}</strong><small>{copy("Completion rate")}</small></div>
        </div>
      </div>
      <div class="profileTabs"><span class:active={!reviews} class:tabHighlight={frameKind === "verify"}>Ads{#if tabClick && !reviews}<i class="tabClick"></i>{/if}</span><span class:active={reviews} class:tabHighlight={reviews}>Reviews{#if tabClick && reviews}<i class="tabClick"></i>{/if}</span></div>
      {#if reviews}
        <div class="reviewPanel" data-testid="bybit-review-panel">
          <div class="reviewFilters"><span class="filterActive">{copy("All")}</span><span>{copy("Good")}</span><span>{copy("Bad")}</span><small>{copy("Illustrative reviews")}</small></div>
          <div class="reviewRow"><span class="reviewAvatar">A</span><div><div class="reviewName"><span>A***</span><span class="textStub"></span></div><p>{copy("Check feedback about payment speed.")}</p></div></div>
          <div class="reviewRow"><span class="reviewAvatar muted">M</span><div><div class="reviewName"><span>M***</span><span class="textStub"></span></div><p>{copy("Read comments about the trading experience.")}</p></div></div>
          <div class="reviewBottom"><span class="textStub"></span><span>1 <span class="pageNext">›</span></span></div>
        </div>
      {:else}
        <div class="adsPanel" data-testid="bybit-ads-panel">
          <div class="adsHeading"><span>{copy("Type / Price")}</span><span>{copy("Available / Limits")}</span><span>{copy("Payment methods")}</span><span>{copy("Trade")}</span></div>
          <div class="adRow" class:offerHighlight={frameKind === "verify" || frameKind === "act"}>
            <div class="adPrice"><span class="adType" class:selling={buying}><img src={assetIcon(offer?.asset ?? step.from)} alt="" />{copy(buying ? "Sell" : "Buy")} {offer?.asset}</span><strong>{number(offer?.price, 6)} <small>{offer?.fiat}</small></strong></div>
            <div class="adLimits"><strong>{number(offer?.available_asset, 8)} {offer?.asset}</strong><small>{number(offer?.min_fiat)} –<br />{number(offer?.max_fiat)} {offer?.fiat}</small></div>
            <div class="paymentMethods"><span>{selectedPayment}</span>{#if (offer?.payment_methods.length ?? 0) > 1}<small>+{(offer?.payment_methods.length ?? 1) - 1}</small>{/if}</div>
            <div class="tradeAction" class:sellAction={!buying} class:actionHighlight={frameKind === "act"} class:clicked data-testid="bybit-trade-action">{action}{#if clicked}<span class="clickRing"></span>{/if}</div>
          </div>
          <div class="otherAds"><span class="skeletonCoin"></span><span class="skeletonLines"><i></i><i></i></span><span class="skeletonLines"><i></i><i></i></span><span class="skeletonPayment"></span><span class="skeletonAction"></span></div>
          <div class="adsBottom"><span>1 <span class="pageNext">›</span></span></div>
        </div>
      {/if}
    </div>
    <div class="scrollTrack"><span></span></div>
  </div>
  <div class="cursorTrack"><svg class="cursor" viewBox="0 0 32 38" fill="none"><path d="M3 2L27 22L15 24L9 35L3 2Z" fill="#17181b" stroke="white" stroke-width="2.5" stroke-linejoin="round" /></svg></div>
</div>

<style>
  .profileCard { --bybit-gold: #f7a600; position: relative; box-sizing: border-box; width: calc(100% - 48px); max-width: 560px; height: 310px; margin: 64px auto 100px; border: 1px solid #e5e7eb; border-radius: 12px; background: #fff; color: #202124; box-shadow: 0 18px 50px #18212e14; font-family: Inter, Arial, sans-serif; container-type: inline-size; overflow: hidden; }
  .bybitNav { height: 36px; display: flex; align-items: center; gap: 16px; padding: 0 18px; background: #17181b; color: #fff; font-size: 10px; }
  .wordmark { font-size: 15px; font-weight: 850; letter-spacing: -.7px; }.wordmark > span { color: var(--bybit-gold); }.profileIcon { width: 18px; height: 18px; margin-left: auto; color: #c5c8ce; }
  .profileViewport { position: relative; height: calc(100% - 36px); overflow: hidden; }.profilePage { margin: 0 18px; transform: translate3d(0, calc(-132px * var(--scroll)), 0); will-change: transform; }
  .profileHeader { height: 132px; box-sizing: border-box; padding-top: 18px; }.identity { display: flex; align-items: center; gap: 10px; }.avatar { flex: 0 0 32px; height: 32px; display: grid; place-items: center; background: #fff3d9; color: #9b6a12; border-radius: 50%; font-size: 14px; font-weight: 700; }.identity > div:nth-child(2) { min-width: 0; }.identity strong { display: block; font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }.identityMeta { display: flex; gap: 8px; align-items: center; margin-top: 5px; color: #90949c; font-size: 8px; }.verified { color: #16886a; }.profileArrow { margin-left: auto; color: #9398a1; font-size: 13px; }
  .statistics { display: flex; margin: 17px 0 0 42px; gap: 40px; }.statistics > div { display: grid; gap: 5px; }.statistics strong { font-size: 15px; font-weight: 600; }.statistics small { color: #8c9099; font-size: 8px; }
  .profileTabs { display: flex; gap: 23px; height: 36px; border-bottom: 1px solid #eceef1; font-size: 11px; font-weight: 600; }.profileTabs > span { position: relative; display: flex; align-items: center; border-bottom: 2px solid transparent; color: #858991; }.profileTabs > span.active { color: #26272b; border-color: var(--bybit-gold); }.tabHighlight { background: linear-gradient(transparent 15%, #f7a60010 15%, #f7a60010 85%, transparent 85%); }
  .tabClick { position: absolute; left: 50%; top: 7px; width: 20px; height: 20px; border: 2px solid #f7a60070; border-radius: 50%; transform: translateX(-50%); }
  .adsHeading, .adRow { display: grid; grid-template-columns: 1.1fr 1.2fr .9fr .9fr; gap: 10px; align-items: center; }.adsHeading { padding: 14px 0 10px; color: #9398a1; font-size: 8px; }.adsHeading > :last-child { text-align: center; }.adRow { position: relative; padding: 12px 0; border-bottom: 1px solid #eceef1; }.adPrice, .adLimits { display: grid; gap: 8px; min-width: 0; }.adPrice > strong { font-size: clamp(9px, 2.6cqw, 14px); font-weight: 650; overflow-wrap: anywhere; }.adPrice strong small { font-size: 8px; font-weight: 400; }.adType { display: flex; align-items: center; gap: 4px; color: #138b6c; font-size: 9px; }.adType.selling { color: #dc5262; }.adType img { width: 13px; height: 13px; }.adLimits strong { font-size: 9px; font-weight: 500; overflow-wrap: anywhere; }.adLimits small { color: #858b95; font-size: 8px; line-height: 1.6; }.paymentMethods { display: flex; flex-wrap: wrap; gap: 4px; align-items: center; min-width: 0; font-size: 8px; }.paymentMethods > span { border-left: 2px solid #f7a600; padding-left: 5px; overflow-wrap: anywhere; line-height: 1.5; }.paymentMethods small { color: #9398a1; }
  .tradeAction { --action-color: #169d7a; position: relative; display: grid; place-items: center; min-height: 29px; padding: 0 5px; box-sizing: border-box; background: var(--action-color); color: white; border-radius: 999px; font-size: 9px; font-weight: 600; text-align: center; overflow-wrap: anywhere; }.sellAction { --action-color: #ef5364; }.actionHighlight { box-shadow: 0 0 0 4px color-mix(in srgb, var(--action-color) 18%, transparent); }.clicked { transform: scale(.96); }.clickRing { position: absolute; width: 24px; height: 24px; border: 2px solid #fff9; border-radius: 50%; opacity: .8; }.offerHighlight::before { content: ""; position: absolute; inset: 3px -7px; border: 1px solid #f7a60050; border-radius: 6px; background: #f7a60005; pointer-events: none; }
  .otherAds { display: flex; align-items: center; gap: 8px; height: 45px; opacity: .6; }.skeletonCoin { width: 12px; height: 12px; flex: 0 0 12px; background: #e5e8ec; border-radius: 50%; }.skeletonLines { display: grid; gap: 5px; flex: 1; }.skeletonLines i { width: 62%; height: 4px; background: #e5e8ec; border-radius: 3px; }.skeletonLines i + i { width: 83%; }.skeletonPayment { width: 16%; height: 5px; background: #e5e8ec; border-radius: 3px; }.skeletonAction { width: 21%; height: 22px; margin-left: 8px; background: #f0f2f5; border-radius: 999px; }
  .adsBottom, .reviewBottom { display: flex; align-items: center; justify-content: space-between; padding-top: 7px; color: #818791; font-size: 8px; }.adsBottom { justify-content: flex-end; }.pageNext { margin-left: 12px; color: #9aa0a9; }
  .reviewFilters { display: flex; align-items: center; gap: 13px; padding: 13px 0 9px; color: #818791; font-size: 9px; }.filterActive { padding: 4px 7px; border-radius: 4px; background: #f4f5f7; color: #30343b; }.reviewFilters small { margin-left: auto; font-size: 7px; color: #a0a5ae; }.reviewRow { display: flex; gap: 9px; padding: 12px 0; border-bottom: 1px solid #eceef1; }.reviewAvatar { display: grid; place-items: center; flex: 0 0 22px; height: 22px; border-radius: 50%; background: #fff3d9; color: #9b6a12; font-size: 9px; }.reviewAvatar.muted { background: #eceff3; color: #868e9c; }.reviewRow > div { flex: 1; min-width: 0; }.reviewName { display: flex; align-items: center; justify-content: space-between; font-size: 8px; color: #676e79; }.textStub { display: inline-block; width: 48px; height: 4px; border-radius: 3px; background: #e9ecf0; }.reviewRow p { margin: 7px 0 0; font-size: 9px; color: #555d69; line-height: 1.4; }.reviewBottom { padding-top: 13px; }
  .scrollTrack { position: absolute; top: 8px; bottom: 8px; right: 5px; width: 3px; background: #f3f4f6; border-radius: 3px; }.scrollTrack > span { display: block; position: absolute; top: calc(var(--scroll) * (100% - 65px)); width: 3px; height: 65px; border-radius: 3px; background: #cdd1d8; }
  .cursorTrack { position: absolute; z-index: 2; top: 36px; left: 0; width: 22px; transform: translate3d(var(--cursor-x), var(--cursor-y), 0); will-change: transform; pointer-events: none; }
  .cursor { display: block; width: 22px; height: 27px; filter: drop-shadow(0 2px 2px #0003); }
  @container (max-width: 360px) { .adsHeading, .adRow { grid-template-columns: 1fr 1.1fr .85fr .85fr; gap: 6px; }.adsHeading { font-size: 6px; }.adLimits strong { font-size: 7px; }.adLimits small, .paymentMethods, .adType { font-size: 7px; }.tradeAction { font-size: 7px; }.identity strong { font-size: 10px; }.statistics { gap: 25px; }.reviewFilters { gap: 10px; }.reviewFilters small { font-size: 6px; }.reviewRow p { font-size: 8px; } }
  @media (max-height: 820px) and (min-width: 761px) { .profileCard { margin-top: 28px; height: 250px; } }
  @media (max-width: 760px) { .profileCard { width: calc(100% - 36px); height: 246px; margin-top: 24px; }.bybitNav { padding: 0 12px; }.profilePage { margin: 0 12px; }.adRow { padding: 10px 0; }.adsHeading { padding-top: 10px; }.otherAds { height: 34px; } }
</style>
