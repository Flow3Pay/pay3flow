<script lang="ts">
  import { locale, t } from "$lib/i18n";
  import type { RouteCandidate } from "$lib/exchange";
  import type { TutorialStep } from "$lib/route-tutorial";
  import { ease, portion, profileMotion } from "../p2p/profile-motion";

  export let route: RouteCandidate;
  export let step: TutorialStep;
  export let frame = 0;
  export let playing = true;
  export let progress = 1;

  $: copy = (key: string, params: Record<string, string | number> = {}) => t(key, params, $locale);
  $: offer = step.offer;
  $: buying = step.kind === "buy";
  $: frameKind = step.frames[frame]?.kind;
  $: motion = profileMotion(frameKind, progress, 126);
  $: action = copy(buying ? "Buy {asset}" : "Sell {asset}", { asset: offer?.asset ?? step.from });
  $: requested = buying ? route.source_payment_method : route.target_payment_method;
  $: selectedPayment = offer?.payment_methods.find(method => method === requested) ?? offer?.payment_methods[0] ?? copy("Check on the platform");
  $: numberLocale = $locale === "ru" ? "ru-RU" : $locale === "hy" ? "hy-AM" : "en-US";
  // On MEXC the sell section follows the buy section; scroll it into view after returning to Ads.
  $: sectionScroll = buying ? 0 : frameKind === "act" ? 1 : frameKind === "verify" ? ease(portion(progress, .28, .55)) : 0;
  const sections = ["Buy from the User", "Sell to the User"];
  function number(value?: string | number | null, decimals = 2) {
    if (value == null || value === "") return "—";
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed.toLocaleString(numberLocale, { maximumFractionDigits: decimals }) : "—";
  }
</script>

<!-- Based on https://www.mexc.com/buy-crypto/merchant/af2545ef4b6c470ebd516aa794bb7a03.
     Offer data comes from the selected route; review placeholders are illustrative. -->
<div class="profileCard" class:paused={!playing} data-testid="mexc-profile-card" data-frame={frame} data-frame-kind={frameKind} data-side={buying ? "buy" : "sell"} data-tab={motion.reviews ? "review" : "ads"} style={`--scroll:${motion.scroll};--cursor-x:${motion.cursorX};--cursor-y:${motion.cursorY}px;--section-scroll:${sectionScroll}`}>
  <div class="mexcNav"><img class="wordmark" src="/icons/venues/mexc-wordmark.svg" alt="" /><span>P2P</span><svg class="profileIcon" viewBox="0 0 24 24" fill="none"><circle cx="12" cy="8" r="3.5" stroke="currentColor" stroke-width="1.7" /><path d="M5 21v-2a7 7 0 0 1 14 0v2" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" /></svg></div>
  <div class="profileViewport">
    <div class="profilePage">
      <div class="profileHeader">
        <div class="identity"><span class="avatar">{offer?.advertiser.nickname.slice(0, 1).toUpperCase() ?? "M"}</span><div><strong data-testid="mexc-advertiser">{offer?.advertiser.nickname}</strong><small>{#if offer?.advertiser.is_verified}<span class="verified">✓ {copy("Verified")}</span>{/if}{copy(offer?.advertiser.is_merchant ? "Merchant" : "Advertiser")}</small></div><span class="profileArrow">↗</span></div>
        <div class="statistics"><div><small>{copy("30D Transactions")}</small><strong>{number(offer?.advertiser.completed_orders_30d, 0)}</strong></div><div><small>{copy("30D Completion Rate")}</small><strong>{offer?.advertiser.completion_rate_30d == null ? "—" : `${number(offer.advertiser.completion_rate_30d * 100)}%`}</strong></div></div>
      </div>
      <div class="profileTabs"><span class:active={!motion.reviews}>Ads{#if motion.tabClick && !motion.reviews}<i class="tabClick"></i>{/if}</span><span class:active={motion.reviews}>Reviews{#if motion.tabClick && motion.reviews}<i class="tabClick"></i>{/if}</span></div>
      {#if motion.reviews}
        <div class="reviewPanel" data-testid="mexc-review-panel">
          <div class="reviewFilters"><span>{copy("All")}</span><span>{copy("Good")}</span><span>{copy("Bad")}</span><small>{copy("Illustrative reviews")}</small></div>
          <div class="reviewRow"><span class="reviewAvatar">A</span><div><small>A***</small><p>{copy("Check feedback about payment speed.")}</p></div><i class="textStub"></i></div>
          <div class="reviewRow"><span class="reviewAvatar muted">M</span><div><small>M***</small><p>{copy("Read comments about the trading experience.")}</p></div><i class="textStub"></i></div>
        </div>
      {:else}
        <div class="adsPanel" data-testid="mexc-ads-panel"><div class="adSections">
          {#each sections as section, index}
            {@const selected = buying ? index === 0 : index === 1}
            <div class="adSection" class:selected class:otherSection={!selected} class:offerHighlight={selected && (frameKind === "verify" || frameKind === "act")} data-section={index === 0 ? "buy" : "sell"}>
              <h3>{section}</h3>
              <div class="adsHeading"><span>{copy("Crypto")}</span><span>{copy("Price")}</span><span>{copy("Available / Limits")}</span><span>{copy("Payment")}</span><span>{copy("Trade")}</span></div>
              {#if selected}
                <div class="adRow">
                  <strong class="crypto">{offer?.asset}</strong>
                  <div class="adPrice"><strong>{number(offer?.price, 6)}</strong><small>{offer?.fiat}</small></div>
                  <div class="adLimits"><strong>{number(offer?.available_asset, 8)} {offer?.asset}</strong><small>{number(offer?.min_fiat)} –<br />{number(offer?.max_fiat)} {offer?.fiat}</small></div>
                  <div class="paymentMethods"><span>{selectedPayment}</span>{#if (offer?.payment_methods.length ?? 0) > 1}<small>+{(offer?.payment_methods.length ?? 1) - 1}</small>{/if}</div>
                  <div class="tradeAction" class:sellAction={!buying} class:actionHighlight={frameKind === "act"} class:clicked={motion.clicked} data-testid="mexc-trade-action">{action}{#if motion.clicked}<span class="clickRing"></span>{/if}</div>
                </div>
              {:else}
                <div class="otherAds"><i></i><i></i><i></i><i></i><i class="skeletonAction"></i></div>
              {/if}
            </div>
          {/each}
        </div></div>
      {/if}
    </div>
    <div class="scrollTrack"><span></span></div>
  </div>
  <div class="cursorTrack"><svg class="cursor" viewBox="0 0 32 38" fill="none"><path d="M3 2L27 22L15 24L9 35L3 2Z" fill="#17181b" stroke="white" stroke-width="2.5" stroke-linejoin="round" /></svg></div>
</div>

<style>
  .profileCard { --mexc-blue: #0057ff; position: relative; box-sizing: border-box; width: calc(100% - 48px); max-width: 560px; height: 310px; margin: 64px auto 100px; border: 1px solid #e5e8ee; border-radius: 12px; background: #fff; color: #15191f; box-shadow: 0 18px 50px #18212e14; font-family: Inter, Arial, sans-serif; container-type: inline-size; overflow: hidden; }
  .mexcNav { box-sizing: border-box; height: 36px; display: flex; align-items: center; gap: 17px; padding: 0 18px; border-bottom: 1px solid #eef0f4; font-size: 10px; }.wordmark { width: 70px; height: 14px; object-fit: contain; }.profileIcon { width: 18px; height: 18px; margin-left: auto; color: #707987; }
  .profileViewport { position: relative; height: calc(100% - 36px); overflow: hidden; }.profilePage { margin: 0 18px; transform: translate3d(0, calc(-132px * var(--scroll)), 0); will-change: transform; }.profileHeader { box-sizing: border-box; height: 132px; padding-top: 18px; }
  .identity { display: flex; align-items: center; gap: 9px; }.avatar { display: grid; place-items: center; flex: 0 0 30px; height: 30px; border-radius: 50%; background: #edf3ff; color: var(--mexc-blue); font-size: 13px; font-weight: 600; }.identity > div { min-width: 0; }.identity strong { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; }.identity small { display: flex; gap: 7px; margin-top: 5px; font-size: 8px; color: #8a93a2; }.verified { color: var(--mexc-blue); }.profileArrow { margin-left: auto; color: #8a93a2; }
  .statistics { display: grid; grid-template-columns: 1fr 1fr; gap: 22px; margin: 21px 0 0 39px; }.statistics > div { display: grid; gap: 5px; }.statistics small { color: #8791a1; font-size: 8px; }.statistics strong { font-size: 15px; font-weight: 650; }
  .profileTabs { display: flex; gap: 23px; height: 36px; border-bottom: 1px solid #edf0f4; font-size: 11px; }.profileTabs > span { position: relative; display: flex; align-items: center; border-bottom: 2px solid transparent; color: #8892a2; }.profileTabs > span.active { color: #15191f; border-color: #15191f; font-weight: 600; }.tabClick { position: absolute; top: 7px; left: 50%; width: 20px; height: 20px; border: 2px solid #0057ff60; border-radius: 50%; transform: translateX(-50%); }
  .adsPanel { height: 180px; margin-top: 10px; overflow: hidden; }.adSections { transform: translate3d(0, calc(-98px * var(--section-scroll)), 0); will-change: transform; }.adSection { box-sizing: border-box; padding: 10px; margin-bottom: 10px; border: 1px solid #edf0f4; border-radius: 8px; }.adSection h3 { margin: 0 0 10px; font-size: 10px; line-height: 12px; font-weight: 600; }.otherSection { height: 88px; }.offerHighlight { border-color: #0057ff50; background: #0057ff03; }
  .adsHeading, .adRow, .otherAds { display: grid; grid-template-columns: .65fr .85fr 1.25fr .9fr .95fr; gap: 6px; align-items: center; }.adsHeading { padding: 6px 4px; background: #f6f7fa; color: #8a93a2; font-size: 7px; }.adsHeading > :last-child { text-align: center; }.adRow { min-height: 60px; padding: 5px 4px; }.crypto { font-size: 9px; font-weight: 500; }.adPrice { display: flex; flex-wrap: wrap; align-items: baseline; gap: 2px; min-width: 0; color: var(--mexc-blue); }.adPrice strong { overflow-wrap: anywhere; font-size: clamp(9px, 2.6cqw, 13px); font-weight: 650; }.adPrice small { font-size: 7px; }.adLimits { display: grid; gap: 6px; min-width: 0; }.adLimits strong { overflow-wrap: anywhere; font-size: 8px; font-weight: 400; }.adLimits small { font-size: 7px; color: #7c8798; line-height: 1.5; }.paymentMethods { display: flex; flex-wrap: wrap; gap: 3px; align-items: center; min-width: 0; font-size: 8px; }.paymentMethods > span { border-left: 2px solid #16b979; padding-left: 4px; overflow-wrap: anywhere; line-height: 1.4; }.paymentMethods small { color: #8a93a2; }
  .tradeAction { --action-color: #16b979; position: relative; display: grid; place-items: center; min-height: 28px; padding: 0 5px; box-sizing: border-box; border-radius: 999px; background: var(--action-color); color: #fff; font-size: 8px; font-weight: 500; text-align: center; overflow-wrap: anywhere; }.sellAction { --action-color: #f6465d; }.actionHighlight { box-shadow: 0 0 0 4px color-mix(in srgb, var(--action-color) 18%, transparent); }.clicked { transform: scale(.96); }.clickRing { position: absolute; width: 24px; height: 24px; border: 2px solid #fff9; border-radius: 50%; }
  .otherAds { height: 20px; padding: 5px 4px; opacity: .6; }.otherAds i { display: block; height: 4px; border-radius: 3px; background: #e7ebf2; }.otherAds .skeletonAction { height: 17px; border-radius: 999px; background: #f0f3f7; }
  .reviewFilters { display: flex; flex-wrap: wrap; align-items: center; gap: 8px 13px; padding: 13px 0 9px; font-size: 9px; color: #818d9e; }.reviewFilters > span:first-child { color: var(--mexc-blue); }.reviewFilters small { margin-left: auto; font-size: 7px; color: #a0a9b7; }.reviewRow { display: flex; align-items: flex-start; gap: 9px; padding: 14px 0; border-bottom: 1px solid #edf0f4; }.reviewAvatar { display: grid; place-items: center; flex: 0 0 22px; height: 22px; border-radius: 50%; background: #edf3ff; color: var(--mexc-blue); font-size: 9px; }.reviewAvatar.muted { background: #f0f3f7; color: #8892a2; }.reviewRow > div { flex: 1; min-width: 0; }.reviewRow small { color: #7c8798; font-size: 8px; }.reviewRow p { margin: 7px 0 0; font-size: 9px; line-height: 1.4; color: #5c6778; }.textStub { width: 35px; height: 4px; margin-top: 4px; border-radius: 3px; background: #e9edf4; }
  .scrollTrack { position: absolute; top: 8px; bottom: 8px; right: 5px; width: 3px; border-radius: 3px; background: #f5f6f8; }.scrollTrack > span { position: absolute; top: calc(var(--scroll) * (100% - 65px)); width: 3px; height: 65px; border-radius: 3px; background: #cdd5e1; }.cursorTrack { position: absolute; z-index: 2; top: 36px; left: 0; width: 22px; transform: translate3d(var(--cursor-x), var(--cursor-y), 0); will-change: transform; pointer-events: none; }.cursor { display: block; width: 22px; height: 27px; filter: drop-shadow(0 2px 2px #0003); }
  @container (max-width: 360px) { .adSection { padding: 8px; }.adSection h3 { margin-bottom: 8px; font-size: 9px; }.adsHeading, .adRow, .otherAds { gap: 4px; }.adsHeading { font-size: 6px; }.crypto, .adLimits strong, .paymentMethods { font-size: 7px; }.adLimits small, .adPrice small { font-size: 6px; }.tradeAction { font-size: 7px; padding: 0 3px; }.identity strong { font-size: 10px; }.reviewFilters { font-size: 8px; gap: 6px 8px; }.reviewFilters small { font-size: 6px; }.reviewRow p { font-size: 8px; } }
  @media (max-height: 820px) and (min-width: 761px) { .profileCard { margin-top: 28px; height: 250px; } }
  @media (max-width: 760px) { .profileCard { width: calc(100% - 36px); height: 246px; margin-top: 24px; }.mexcNav { padding: 0 12px; }.profilePage { margin: 0 12px; }.adsPanel { height: 160px; margin-top: 8px; } }
</style>
