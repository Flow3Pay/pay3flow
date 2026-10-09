<script lang="ts">
  import { fiatFlagUrl } from "$lib/currency-flags";
  import { assetIcon, venueIcon } from "$lib/icons";
  import { locale, t } from "$lib/i18n";
  import { tutorialMoney, type TutorialStep } from "$lib/route-tutorial";
  import type { RouteCandidate } from "$lib/exchange";
  import { ease, portion } from "../p2p/profile-motion";
  import { swapGuideAction } from "./frames";

  export let route: RouteCandidate;
  export let step: TutorialStep;
  export let frame = 0;
  export let playing = true;
  export let progress = 1;
  $: copy = (key: string, params: Record<string, string | number> = {}) => t(key, params, $locale);
  $: provider = step.provider.toLowerCase();
  $: wallet = provider === "symbiosis" || provider === "cow-swap";
  $: vertical = wallet || provider === "bncex";
  $: kind = step.frames[frame]?.kind;
  $: input = step.amount ?? (step.from === route.source_currency ? tutorialMoney(route.source_amount_minor, route.source_currency, route.source_amount) : undefined);
  $: fields = [
    { side: "send", currency: step.from, value: amount(input, step.from), network: fiatFlagUrl(step.from) ? route.source_payment_method : step.network },
    { side: "receive", currency: step.to, value: amount(step.output, step.to), network: fiatFlagUrl(step.to) ? route.target_payment_method : step.targetNetwork ?? step.network },
  ];
  $: action = copy(swapGuideAction(provider));
  $: travel = ease(portion(progress, 0, .8));
  $: cursorX = kind === "open" ? 88 - 13 * travel : kind === "verify" ? 75 - 56 * travel : kind === "review" ? 19 + (vertical ? 0 : 48) * travel : (vertical ? 19 : 67) + (74 - (vertical ? 19 : 67)) * travel;
  $: cursorY = kind === "open" ? 15 + 24 * travel : kind === "verify" ? 39 + 2 * travel : kind === "review" ? 41 + (vertical ? 24 : 0) * travel : (vertical ? 65 : 41) + (88 - (vertical ? 65 : 41)) * travel;
  $: clicked = kind === "act" && progress >= .8;
  function amount(value: string | undefined, currency: string) {
    if (!value || value === "—") return "—";
    const raw = value.endsWith(` ${currency}`) ? value.slice(0, -currency.length - 1) : value;
    const numeric = Number(raw.replace(/,/g, ""));
    return Number.isFinite(numeric) ? numeric.toLocaleString($locale === "ru" ? "ru-RU" : $locale === "hy" ? "hy-AM" : "en-US", { maximumFractionDigits: 8 }) : raw;
  }
</script>

<!-- Venue layouts follow their public exchange forms. Wallet controls are illustrative only. -->
<div class="swapCard" class:vertical class:paused={!playing} class:bncex={provider === "bncex"} class:bitcoin={provider === "bitcoin-center"} class:dzengi={provider === "dzengi"} class:symbiosis={provider === "symbiosis"} class:cow={provider === "cow-swap"} data-testid={`${provider}-swap-card`} data-frame-kind={kind} style={`--cursor-x:${cursorX}cqw;--cursor-y:${cursorY}cqh`}>
  <div class="venueHeader"><img src={venueIcon(provider)} alt="" /><strong>{provider === "bitcoin-center" ? "Bitcoincenter.am" : provider === "cow-swap" ? "CoW Swap" : provider === "dzengi" ? "DZENGI" : provider === "bncex" ? "bncex" : "symbiosis"}</strong><span>{wallet ? copy("Connect wallet") : copy("Exchange")}</span></div>
  <div class="body">
    {#if provider === "bncex"}<div class="buySell"><span class:active={Boolean(fiatFlagUrl(step.from))}>{copy("Buy")}</span><span class:active={!fiatFlagUrl(step.from)}>{copy("Sell")}</span></div>
    {:else if provider === "symbiosis"}<h3>Swap &amp; Bridge <span>⚙</span></h3>
    {:else if provider === "cow-swap"}<div class="cowTabs"><span>Swap</span><span>Limit</span><span>TWAP</span><i>⚙</i></div>
    {:else if provider === "dzengi"}<h3 class="pair">{step.from} → {step.to}</h3>
    {:else}<div class="sectionNumbers"><span>01 · {copy("You send")}</span><span>02 · {copy("You receive")}</span></div>{/if}
    <div class="fields">
      {#each fields as field, index}
        <div class="field" class:amountHighlight={kind === "verify" && index === 0 || kind === "review" && index === 1} data-side={field.side}>
          <small class="fieldLabel">{provider === "bncex" ? copy(index === 0 ? "Give" : "Receive") : provider === "symbiosis" ? copy(index === 0 ? "From" : "To") : copy(index === 0 ? "You send" : "You receive")}</small>
          <div class="fieldValue"><strong class="amount" data-testid={`${provider}-${field.side}-amount`}>{field.value}</strong><div class="currency" class:selectHighlight={kind === "open"}><img src={fiatFlagUrl(field.currency) ?? assetIcon(field.currency)} alt="" /><div><strong>{field.currency}</strong>{#if field.network && provider !== "dzengi"}<small data-testid={`${provider}-${field.side}-network`}>{field.network}</small>{/if}</div><span>⌄</span></div></div>
          {#if provider === "bncex"}<div class="networkSelector" class:selectHighlight={kind === "open"}><span>{field.network ?? field.currency}</span><span>▾</span></div>{/if}
          {#if provider === "bitcoin-center"}<div class="selectedCurrency"><span>✓</span><span>{field.network ?? field.currency}</span></div>{/if}
        </div>
      {/each}
      <span class="directionArrow">{vertical ? "↓" : "⇄"}</span>
    </div>
    <div class="quoteLine" class:quoteHighlight={kind === "review"}>{#if provider === "bncex"}<span>{copy("How does it calculate")}</span><span>{copy("Check quote timer")}</span>{:else if provider === "bitcoin-center"}<span>03 · {copy("Order setup")}</span><span>{copy("Check the exchange terms")}</span>{:else if provider === "dzengi"}<span>{copy("Indicative quote · check the price on Dzengi")}</span>{:else}<span>{copy("Fees and slippage")}</span><span>{copy("Check on the platform")}</span>{/if}</div>
    <div class="exchangeAction" class:actionHighlight={kind === "act"} class:clicked data-testid={`${provider}-exchange-action`}>{action}{#if clicked}<i></i>{/if}</div>
  </div>
  <div class="cursorTrack"><svg viewBox="0 0 32 38" fill="none"><path d="M3 2L27 22L15 24L9 35L3 2Z" fill="#151c25" stroke="white" stroke-width="2.5" stroke-linejoin="round" /></svg></div>
</div>

<style>
  .swapCard { --accent: #187260; --surface: #f1f4f4; --border: #e0e6e9; --muted: #89929c; position: relative; overflow: hidden; box-sizing: border-box; width: calc(100% - 48px); max-width: 520px; height: 310px; margin: 64px auto 100px; container-type: size; border: 1px solid var(--border); border-radius: 18px; background: #fff; color: #172331; font-family: Inter, Arial, sans-serif; box-shadow: 0 18px 50px #20304018; }.venueHeader { height: 36px; display: flex; align-items: center; gap: 6px; padding: 0 16px; border-bottom: 1px solid var(--border); font-size: 12px; }.venueHeader img { width: 19px; height: 19px; }.venueHeader > span { margin-left: auto; padding: 4px 7px; background: var(--surface); border-radius: 7px; color: var(--accent); font-size: 8px; }.body { box-sizing: border-box; height: calc(100% - 36px); padding: 13px 16px; display: flex; flex-direction: column; gap: 9px; }h3 { display: flex; justify-content: space-between; margin: 0; font-size: 12px; }h3 > span { color: var(--muted); }.fields { position: relative; display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: 14px; flex: 1; min-height: 0; }.field { display: flex; flex-direction: column; justify-content: center; min-width: 0; box-sizing: border-box; padding: 12px; border: 1px solid var(--border); border-radius: 11px; background: var(--surface); }.fieldLabel { display: block; color: var(--muted); font-size: 8px; margin-bottom: 8px; }.fieldValue { display: flex; flex-wrap: wrap; gap: 12px; align-items: center; justify-content: space-between; min-width: 0; }.amount { min-width: 0; overflow-wrap: anywhere; font-size: clamp(12px, 4cqw, 21px); font-weight: 600; font-variant-numeric: tabular-nums; }.currency { display: flex; gap: 5px; align-items: center; min-width: 0; border-radius: 20px; padding: 4px; }.currency img { flex: 0 0 auto; width: 19px; height: 19px; border-radius: 50%; object-fit: cover; }.currency > div { display: grid; min-width: 0; gap: 3px; }.currency strong { font-size: 10px; }.currency small { color: var(--muted); font-size: 7px; max-width: 105px; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }.currency > span { color: var(--muted); font-size: 11px; }.directionArrow { position: absolute; left: 50%; top: 50%; display: grid; place-items: center; width: 24px; height: 24px; background: #fff; border: 2px solid #fff; border-radius: 50%; transform: translate(-50%, -50%); color: var(--accent); font-size: 14px; }.quoteLine { min-height: 18px; display: flex; align-items: center; justify-content: space-between; flex-wrap: wrap; gap: 4px; color: var(--muted); font-size: 8px; }.exchangeAction { position: relative; flex: 0 0 30px; display: grid; place-items: center; padding: 0 7px; border-radius: 8px; background: var(--accent); color: #fff; font-size: 10px; font-weight: 600; }.amountHighlight { border-color: var(--accent); box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 15%, transparent); }.selectHighlight { box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 35%, transparent); }.quoteHighlight { color: var(--accent); }.actionHighlight { box-shadow: 0 0 0 4px color-mix(in srgb, var(--accent) 20%, transparent); }.clicked { transform: scale(.99); }.exchangeAction i { position: absolute; left: 74%; top: 4px; width: 18px; height: 18px; border: 2px solid #ffffffaa; border-radius: 50%; }.vertical .fields { grid-template-columns: 1fr; grid-template-rows: repeat(2, minmax(0, 1fr)); gap: 8px; }.vertical .field { min-height: 0; padding: 8px 12px; }.vertical .fieldLabel { margin-bottom: 4px; }.vertical .fieldValue { flex-wrap: nowrap; gap: 7px; }.vertical .directionArrow { background: var(--surface); border-color: #fff; }.vertical .amount { flex: 1; font-size: clamp(13px, 4cqw, 20px); }
  .bncex { --accent: #00efb2; --surface: #0f1514; --border: #26332f; --muted: #8d9692; background: #080d0c; color: #e7eae8; max-width: 450px; border-radius: 14px; }.bncex .venueHeader strong { color: var(--accent); }.bncex .exchangeAction { color: #0b1511; border-radius: 6px; }.buySell { display: flex; border: 1px solid var(--border); border-radius: 6px; padding: 3px; font-size: 9px; }.buySell > span { flex: 1; padding: 4px; text-align: center; color: var(--muted); }.buySell .active { color: #eee; background: #171b1a; border-radius: 5px; }.bncex .body { padding: 8px 12px; gap: 5px; }.bncex .fields { gap: 5px; }.bncex .field { padding: 4px 8px; border-color: #00efb240; border-radius: 9px; }.bncex .fieldLabel { margin-bottom: 3px; font-size: 7px; text-transform: uppercase; }.bncex .currency { padding: 2px; }.bncex .quoteLine { min-height: 10px; font-size: 7px; }.bncex .amount { font-size: 14px; }.bncex .buySell { padding: 2px; }.bncex .buySell > span { padding: 3px; }.bncex .networkSelector { margin-top: 3px; padding: 3px 5px; }.bncex .currency img { width: 15px; height: 15px; }.bncex .currency small { display: none; }.networkSelector { display: flex; justify-content: space-between; gap: 8px; margin-top: 5px; padding: 5px 7px; background: #090d0b; border: 1px solid var(--border); border-radius: 5px; font-size: 8px; }.bncex .directionArrow { display: none; }
  .bitcoin { --accent: #ed6500; --surface: #f8faff; --border: #e0e5ee; --muted: #7285a1; border-radius: 12px; }.bitcoin .field { background: #fff; }.sectionNumbers { display: grid; grid-template-columns: 1fr 1fr; gap: 14px; font-size: 9px; font-weight: 600; }.selectedCurrency { display: flex; gap: 5px; align-items: center; color: var(--accent); margin-top: 13px; font-size: 8px; padding: 8px 5px; border: 1px solid #ed650044; border-radius: 6px; background: #fff4ec; }.selectedCurrency > span:last-child { overflow-wrap: anywhere; }.bitcoin .directionArrow { border: 1px solid #ed650066; border-radius: 8px; }
  .dzengi { --accent: #1b715f; --surface: #f0f4f4; --muted: #859590; }.dzengi .venueHeader { color: var(--accent); letter-spacing: 2px; }.dzengi .venueHeader img { display: none; }.dzengi .pair { justify-content: center; font-size: 16px; }.dzengi .field { border: 0; }.dzengi .fieldLabel { display: none; }.dzengi .fieldValue { flex-direction: column-reverse; align-items: flex-start; gap: 18px; }.dzengi .currency { padding: 0; }.dzengi .exchangeAction { border-radius: 5px; }.dzengi .quoteLine { justify-content: center; font-size: 7px; }
  .symbiosis { --accent: #111318; --surface: #f7f8fa; --muted: #969dab; max-width: 370px; border-radius: 22px; }.symbiosis .field { border-radius: 13px; }.symbiosis .fieldLabel { color: #959eae; }.symbiosis .currency { background: #fff; }.symbiosis .exchangeAction { border-radius: 10px; }
  .cow { --accent: #003e88; --surface: #f2f2f2; --border: #e7eef3; --muted: #7587a0; max-width: 400px; border: 4px solid #a0e8ff; border-radius: 20px; color: #00244f; }.cow .venueHeader { background: #a0e8ff; padding: 0 12px; border: 0; }.cow .venueHeader > span { background: #003e88; color: #a0e8ff; }.cowTabs { display: flex; align-items: center; gap: 11px; font-size: 9px; }.cowTabs > span:first-child { background: var(--surface); border-radius: 9px; padding: 4px 7px; }.cowTabs i { margin-left: auto; color: var(--muted); font-style: normal; }.cow .field { border-radius: 14px; }.cow .field:last-of-type { background: white; }.cow .fieldLabel { display: none; }.cow .currency { background: #fff; padding: 4px 8px; box-shadow: 0 2px 6px #003e8810; }.cow .exchangeAction { color: #87dcff; border-radius: 12px; }
  .cursorTrack { position: absolute; z-index: 2; left: 0; top: 0; width: 22px; transform: translate3d(var(--cursor-x), var(--cursor-y), 0); will-change: transform; pointer-events: none; }.cursorTrack svg { display: block; width: 22px; height: 27px; filter: drop-shadow(0 2px 2px #0004); }
  @container (max-width: 360px) { .body { padding: 10px 12px; gap: 7px; }.venueHeader { padding: 0 12px; font-size: 10px; }.venueHeader > span { font-size: 7px; }.field { padding: 8px; }.fields { gap: 10px; }.currency img { width: 16px; height: 16px; }.currency strong { font-size: 9px; }.currency small { font-size: 6px; max-width: 80px; }.fieldValue { gap: 7px; }.quoteLine { font-size: 7px; }.sectionNumbers { font-size: 8px; gap: 10px; }.selectedCurrency { margin-top: 9px; font-size: 7px; }.exchangeAction { font-size: 9px; flex-basis: 28px; }.dzengi .fieldValue { gap: 12px; } }
  @media (max-height: 820px) and (min-width: 761px) { .swapCard { margin-top: 28px; height: 250px; } }
  @media (max-width: 760px) { .swapCard { width: calc(100% - 36px); margin-top: 24px; height: 246px; } }
</style>
