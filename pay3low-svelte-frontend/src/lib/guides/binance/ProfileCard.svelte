<script lang="ts">
  import { locale, t } from "$lib/i18n";
  import type { RouteCandidate } from "$lib/exchange";
  import type { TutorialStep } from "$lib/route-tutorial";
  import { ease, portion } from "../p2p/profile-motion";
  import { assetIcon, venueIcon } from "$lib/icons";

  export let route: RouteCandidate;
  export let step: TutorialStep;
  export let frame = 0;
  export let playing = true;
  export let progress = 1;
  $: copy = (key: string, params: Record<string, string | number> = {}) => t(key, params, $locale);
  $: offer = step.offer;
  $: buying = step.kind === "buy";
  $: kind = step.frames[frame]?.kind;
  $: details = kind === "review" ? progress >= .24 : kind === "verify" && progress < .24;
  $: scroll = kind === "verify" ? ease(portion(progress, .28, .6)) : kind === "act" ? 1 : 0;
  $: travel = ease(portion(progress, .35, .85));
  $: actionY = buying ? 73 : 147;
  $: cursorX = kind === "open" ? 85 - 45 * ease(portion(progress, 0, .65)) : kind === "review" ? 40 + 54 * ease(portion(progress, 0, .2)) - 17 * travel : kind === "verify" ? 77 + 10 * travel : 87;
  $: cursorY = kind === "open" ? 24 + 36 * ease(portion(progress, 0, .65)) : kind === "review" ? 60 + 32 * ease(portion(progress, 0, .2)) - 75 * travel : kind === "verify" ? 17 + (actionY - 17) * travel : actionY;
  $: clicked = kind === "act" && progress >= .5;
  $: requested = buying ? route.source_payment_method : route.target_payment_method;
  $: payment = offer?.payment_methods.find(method => method === requested) ?? offer?.payment_methods[0] ?? copy("Check on the platform");
  $: stats = [
    ["30D Transactions", number(offer?.advertiser.completed_orders_30d)],
    ["30D Completion Rate", offer?.advertiser.completion_rate_30d == null ? "—" : `${number(offer.advertiser.completion_rate_30d * 100)}%`],
    ["Average release time", "—"], ["Average payment time", "—"],
  ];
  function number(value?: string | number | null) {
    return value == null ? "—" : Number(value).toLocaleString($locale === "ru" ? "ru-RU" : $locale === "hy" ? "hy-AM" : "en-US", { maximumFractionDigits: 6 });
  }
</script>

<!-- Binance uses an ellipsis / More details trading-information window, not a Reviews tab. -->
<div class="profileCard" class:paused={!playing} data-testid="binance-profile-card" data-frame-kind={kind} data-side={buying ? "buy" : "sell"} style={`--scroll:${scroll};--cursor-x:${cursorX}cqw;--cursor-y:${cursorY}px`}>
  <div class="nav"><img src={venueIcon("binance")} alt="" /><strong>BINANCE</strong><span>P2P</span><svg viewBox="0 0 24 24" fill="none"><circle cx="12" cy="8" r="3.5" stroke="currentColor" stroke-width="1.6" /><path d="M5 21v-2a7 7 0 0 1 14 0v2" stroke="currentColor" stroke-width="1.6" /></svg></div>
  <div class="viewport"><div class="page">
    <div class="header"><div class="identity"><span class="avatar">{offer?.advertiser.nickname.slice(0, 1).toUpperCase()}</span><div><strong>{offer?.advertiser.nickname}</strong><small>{#if offer?.advertiser.is_verified}✓ {copy("Verified")}{/if}</small></div></div>
      <div class="statistics">{#each stats.slice(0, 2) as [label, value]}<div><small>{copy(label)}</small><strong>{value}</strong></div>{/each}<span class="more" class:highlight={kind === "review" && progress < .24}>···</span></div>
      {#if kind === "review" && progress >= .12 && progress < .24}<span class="tooltip">{copy("More details")}</span>{/if}
    </div>
    <h3 class="adsTitle">{copy("Active advertisements")}</h3>
    <div class="ads" data-testid="binance-ads">
      {#each [true, false] as buy}
        {@const selected = buy === buying}
        <div class="adSection" class:highlight={selected && (kind === "verify" || kind === "act")}>
          <h4>{copy(buy ? "Active buy advertisements" : "Active sell advertisements")}</h4>
          <div class="adRow"><span class="asset"><img src={assetIcon(offer?.asset ?? step.from)} alt="" />{offer?.asset}</span><div class="price"><strong>{selected ? number(offer?.price) : "—"}</strong><small>{offer?.fiat}</small></div><div class="limits"><span>{selected ? number(offer?.available_asset) : "—"} {offer?.asset}</span><small>{selected ? `${number(offer?.min_fiat)} – ${number(offer?.max_fiat)}` : "—"} {offer?.fiat}</small></div><span class="payment">{selected ? payment : "—"}</span><div class="tradeAction" class:sell={!buy} class:clicked={selected && clicked} data-testid={selected ? "binance-trade-action" : undefined}>{copy(buy ? "Buy {asset}" : "Sell {asset}", { asset: offer?.asset ?? step.from })}</div></div>
        </div>
      {/each}
    </div>
  </div></div>
  {#if details}<div class="shade"><div class="details" data-testid="binance-details"><div class="detailsTitle"><strong>{copy("Trade information")}</strong><span class:highlight={kind === "verify"}>×</span></div>{#each stats as [label, value]}<div class="detailRow"><span>{copy(label)}</span><strong>{value}</strong></div>{/each}<div class="detailHint">{copy("Check the counterparty's trading history before choosing an advertisement.")}</div></div></div>{/if}
  <div class="cursorTrack"><svg viewBox="0 0 32 38" fill="none"><path d="M3 2L27 22L15 24L9 35L3 2Z" fill="#f0b90b" stroke="#fff" stroke-width="2.5" stroke-linejoin="round" /></svg></div>
</div>

<style>
  .profileCard { position: relative; width: calc(100% - 48px); max-width: 560px; height: 310px; margin: 64px auto 100px; box-sizing: border-box; container-type: inline-size; border: 1px solid #30363f; border-radius: 10px; overflow: hidden; background: #181a20; color: #eaecef; font-family: Inter, Arial, sans-serif; box-shadow: 0 18px 50px #0002; }
  .nav { height: 36px; box-sizing: border-box; display: flex; gap: 5px; align-items: center; padding: 0 15px; border-bottom: 1px solid #2b3139; color: #f0b90b; font-size: 11px; }.nav img { width: 17px; height: 17px; }.nav > span { color: #eaecef; margin-left: 17px; font-size: 10px; }.nav svg { width: 17px; height: 17px; color: #848e9c; margin-left: auto; }
  .viewport { height: calc(100% - 36px); overflow: hidden; }.page { margin: 0 14px; transform: translate3d(0, calc(-132px * var(--scroll)), 0); will-change: transform; }.header { position: relative; height: 132px; box-sizing: border-box; padding-top: 16px; background: radial-gradient(ellipse at top right, #f0b90b15, transparent 65%); }.identity { display: flex; align-items: center; gap: 9px; }.avatar { display: grid; place-items: center; flex: 0 0 29px; height: 29px; background: #464fc9; border-radius: 50%; font-size: 13px; }.identity > div { min-width: 0; }.identity strong { display: block; font-size: 12px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }.identity small { display: block; margin-top: 5px; color: #0ecb81; font-size: 8px; }.statistics { display: flex; align-items: center; gap: 8px; margin-top: 15px; }.statistics > div { flex: 1; display: grid; gap: 8px; border: 1px solid #30363f; border-radius: 8px; padding: 10px; text-align: center; }.statistics small { font-size: 8px; color: #848e9c; }.statistics strong { font-size: 13px; }.more { display: grid; place-items: center; width: 22px; height: 22px; border: 1px solid #848e9c; border-radius: 50%; font-size: 15px; letter-spacing: 1px; }.tooltip { position: absolute; right: 0; top: 112px; padding: 5px; background: #2b3139; border-radius: 4px; font-size: 8px; }
  .adsTitle { height: 28px; margin: 0; display: flex; align-items: center; font-size: 10px; border-bottom: 2px solid #f0b90b; width: fit-content; }.adSection { height: 74px; padding: 6px 0; box-sizing: border-box; border-bottom: 1px solid #30363f; }.adSection h4 { font-size: 9px; margin: 0 0 8px; }.adRow { display: grid; grid-template-columns: .7fr .7fr 1.4fr 1fr 1fr; gap: 6px; align-items: center; font-size: 8px; }.asset { display: flex; align-items: center; gap: 4px; }.asset img { width: 13px; height: 13px; }.price { display: flex; flex-wrap: wrap; align-items: baseline; gap: 3px; }.price strong { font-size: 11px; }.price small { font-size: 7px; }.limits { display: grid; gap: 4px; min-width: 0; overflow-wrap: anywhere; }.limits small { font-size: 7px; color: #848e9c; }.payment { border-left: 2px solid #0ecb81; padding-left: 4px; min-width: 0; overflow-wrap: anywhere; }.tradeAction { background: #0ecb81; color: #fff; min-height: 24px; display: grid; place-items: center; text-align: center; border-radius: 4px; font-size: 8px; padding: 0 3px; }.sell { background: #f6465d; }.clicked { transform: scale(.96); box-shadow: 0 0 0 3px #eaecef33; }.highlight { box-shadow: 0 0 0 2px #f0b90b55; }
  .shade { position: absolute; inset: 36px 0 0; display: grid; place-items: center; background: #0009; }.details { width: 74%; max-width: 330px; box-sizing: border-box; padding: 14px; border-radius: 5px; background: #1e2329; }.detailsTitle { display: flex; align-items: center; justify-content: space-between; gap: 10px; margin-bottom: 13px; font-size: 11px; }.detailsTitle > span { color: #848e9c; font-size: 20px; line-height: 16px; border-radius: 3px; }.detailRow { display: flex; gap: 8px; justify-content: space-between; padding: 9px 0; font-size: 9px; }.detailRow strong { white-space: nowrap; }.detailHint { margin-top: 7px; padding-top: 9px; border-top: 1px solid #30363f; color: #848e9c; font-size: 8px; line-height: 1.5; }
  .cursorTrack { position: absolute; z-index: 3; left: 0; top: 36px; width: 22px; transform: translate3d(var(--cursor-x), var(--cursor-y), 0); will-change: transform; pointer-events: none; }.cursorTrack svg { display: block; width: 22px; height: 27px; filter: drop-shadow(0 2px 2px #0005); }
  @container (max-width: 360px) { .page { margin: 0 10px; }.adRow { font-size: 7px; gap: 4px; }.asset { gap: 2px; }.asset img { width: 10px; height: 10px; }.price strong { font-size: 9px; }.price small, .limits small { font-size: 6px; }.tradeAction { font-size: 7px; }.statistics small { font-size: 7px; }.detailRow { font-size: 8px; padding: 7px 0; }.details { padding: 12px; }.detailsTitle { font-size: 10px; margin-bottom: 9px; }.detailHint { font-size: 7px; } }
  @media (max-height: 820px) and (min-width: 761px) { .profileCard { margin-top: 28px; height: 250px; } }
  @media (max-width: 760px) { .profileCard { width: calc(100% - 36px); height: 246px; margin-top: 24px; } }
</style>
