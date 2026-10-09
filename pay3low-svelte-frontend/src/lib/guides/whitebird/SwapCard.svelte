<script lang="ts">
  import { fiatFlagUrl } from "$lib/currency-flags";
  import { assetIcon } from "$lib/icons";
  import { locale, t } from "$lib/i18n";
  import { tutorialMoney, type TutorialStep } from "$lib/route-tutorial";
  import type { RouteCandidate } from "$lib/exchange";

  export let route: RouteCandidate;
  export let step: TutorialStep;
  export let frame = 0;
  export let playing = true;

  $: copy = (key: string) => t(key, {}, $locale);
  $: numberLocale = $locale === "en" ? "en-US" : $locale === "ru" ? "ru-RU" : "hy-AM";
  const currencyIcon = (asset: string) => fiatFlagUrl(asset) ?? assetIcon(asset);
  const assetNames: Record<string, string> = { USDT: "Tether", USDC: "USD Coin", BTC: "Bitcoin", ETH: "Ethereum", TRX: "TRON", BNB: "BNB", GRAM: "TON", TON: "TON", SOL: "SOLANA" };
  function amount(value: string | undefined, currency: string) {
    if (!value) return "—";
    const numeric = value.endsWith(` ${currency}`) ? value.slice(0, -currency.length - 1) : value;
    const parsed = Number(numeric.replace(/,/g, ""));
    return numeric.trim() && Number.isFinite(parsed)
      ? parsed.toLocaleString(numberLocale, { maximumFractionDigits: 8 })
      : numeric;
  }
  // A direct crypto sale has no entry offer, but its input amount is still known.
  // Intermediate balances stay unspecified until the preceding exchange completes.
  $: input = step.amount ?? (step.from === route.source_currency && !route.entry_offer_snapshot && !route.route_provider
    ? tutorialMoney(route.source_amount_minor, route.source_currency, route.source_amount)
    : undefined);
  $: fields = [
    { side: "send", label: copy("Whitebird sell"), currency: step.from, value: amount(input, step.from), subtitle: fiatFlagUrl(step.from) ? undefined : step.network ?? assetNames[step.from] },
    { side: "receive", label: copy("Whitebird receive"), currency: step.to, value: amount(step.output, step.to), subtitle: fiatFlagUrl(step.to) ? undefined : step.network ?? assetNames[step.to] },
  ];
  $: rate = Number(step.offer?.price);
  $: quote = step.offer && Number.isFinite(rate) && rate > 0
    ? `1 ${step.offer.asset} ≈ ${rate.toLocaleString(numberLocale, { maximumFractionDigits: 6 })} ${step.offer.fiat}`
    : `${step.from} → ${step.to}`;
  $: frameKind = step.frames[frame]?.kind;
  $: cursorX = frameKind === "open" ? "37%" : frameKind === "verify" ? "18%" : frameKind === "act" ? "85%" : "66%";
  $: cursorY = frameKind === "act" ? "85%" : "42%";
</script>

<!-- Based on the public swap card at https://whitebird.io/exchanger. -->
<div class="swapCard" class:paused={!playing} data-testid="whitebird-swap-card" data-frame={frame} data-frame-kind={frameKind} style={`--cursor-x:${cursorX};--cursor-y:${cursorY}`}>
  <div class="quote">{quote}</div>
  <div class="fields">
    {#each fields as field, index (field.side)}
      {#if index === 1}
        <div class="swapDirection">
          <svg viewBox="0 0 24 24" fill="none"><path d="M5 8h14m-4-4 4 4-4 4M19 16H5m4-4-4 4 4 4" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" /></svg>
        </div>
      {/if}
      <div class="field" data-side={field.side}>
        <small class="fieldLabel">{field.label}</small>
        <div class="fieldControl" class:highlight={frameKind === "verify" || (frameKind === "review" && index === 1)}>
          <strong class="amount" data-testid={`whitebird-${field.side}-amount`}>{field.value}</strong>
          <div class="currency" class:selectHighlight={frameKind === "open"}>
            <img src={currencyIcon(field.currency)} alt="" />
            <div class="currencyText"><strong>{field.currency}</strong>{#if field.subtitle}<small title={field.subtitle}>{field.subtitle}</small>{/if}</div>
            <svg class="chevron" viewBox="0 0 14 8" fill="none"><path d="m1 1 6 6 6-6" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" /></svg>
          </div>
        </div>
      </div>
    {/each}
  </div>
  <div class="fees" class:highlight={frameKind === "verify"}>
    <span>{copy("Service commission")} <span class="info">i</span></span>
    <strong>{copy("Check on Whitebird")}</strong>
  </div>
  <div class="cardFooter">
    <span>{copy("Whitebird verification required")}</span>
    <div class="exchangeAction" class:actionHighlight={frameKind === "act"}>{copy("Whitebird exchange")}</div>
  </div>
  <svg class="cursor" viewBox="0 0 32 38" fill="none"><path d="M3 2L27 22L15 24L9 35L3 2Z" fill="#0169ff" stroke="white" stroke-width="2.5" stroke-linejoin="round" /></svg>
</div>

<style>
  .swapCard { --wb-blue: #0169ff; position: relative; width: calc(100% - 48px); max-width: 520px; margin: 112px auto 100px; padding: 22px; box-sizing: border-box; border: 1px solid #ebedf0; border-radius: 24px; background: #fff; color: #25262c; font-family: Inter, Arial, sans-serif; box-shadow: 0 18px 50px #1c355c18, 0 3px 10px #1c355c06; container-type: inline-size; animation: cardIn .7s cubic-bezier(.22,1,.36,1) both; }
  .quote { padding-bottom: 16px; border-bottom: 1px solid #f0f1f3; font-size: 11px; font-weight: 600; }
  .fields { display: grid; grid-template-columns: minmax(0, 1fr) 24px minmax(0, 1fr); gap: 8px; align-items: end; margin-top: 16px; }
  .field { min-width: 0; }
  .fieldLabel { display: block; margin-bottom: 5px; color: #83899c; font-size: 10px; font-weight: 500; }
  .fieldControl { display: flex; align-items: center; gap: 5px; min-height: 44px; padding: 5px 8px; border: 1px solid #dce0e7; border-radius: 8px; box-sizing: border-box; background: #fff; transition: border-color .4s, box-shadow .4s; }
  .amount { flex: 1; min-width: 0; overflow-wrap: anywhere; font-size: clamp(11px, 3.1cqw, 16px); font-weight: 600; line-height: 1.25; font-variant-numeric: tabular-nums; }
  .currency { display: flex; flex: 0 1 auto; align-items: center; gap: 5px; min-width: 0; padding: 3px; margin-right: -3px; border-radius: 5px; transition: background .4s, box-shadow .4s; }
  .currency img { flex: 0 0 auto; width: 19px; height: 19px; border-radius: 50%; object-fit: cover; }
  .currencyText { display: grid; gap: 2px; min-width: 0; }
  .currencyText strong { font-size: 11px; line-height: 1; font-weight: 600; }
  .currencyText small { max-width: 72px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: #858b9c; font-size: 7px; line-height: 1.2; }
  .chevron { flex: 0 0 auto; width: 9px; height: 6px; margin-left: 2px; color: #858b9c; }
  .swapDirection { display: grid; place-items: center; width: 24px; height: 24px; margin-bottom: 10px; border-radius: 50%; background: #fff; color: var(--wb-blue); box-shadow: 0 3px 12px #25262c15; }
  .swapDirection svg { width: 16px; height: 16px; }
  .fees { display: flex; flex-wrap: wrap; align-items: center; gap: 5px 9px; margin-top: 22px; padding: 11px 10px; border: 1px solid transparent; border-radius: 8px; background: #f7f8fa; font-size: 9px; transition: border-color .4s, box-shadow .4s; }
  .fees > span { display: flex; align-items: center; gap: 4px; color: #83899c; }
  .fees > strong { font-weight: 400; color: #535760; }
  .info { display: grid; width: 10px; height: 10px; place-items: center; border: 1px solid #969bac; border-radius: 50%; font-size: 7px; font-style: normal; }
  .cardFooter { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-top: 17px; }
  .cardFooter > span { max-width: 210px; color: var(--wb-blue); font-size: 8px; line-height: 1.5; }
  .exchangeAction { flex: 0 0 auto; display: grid; place-items: center; min-height: 32px; padding: 0 13px; border: 1px solid transparent; border-radius: 8px; background: var(--wb-blue); color: #fff; font-size: 11px; font-weight: 600; transition: box-shadow .4s; }
  .highlight { border-color: var(--wb-blue); box-shadow: 0 0 0 3px #0169ff14; }
  .currency.selectHighlight { background: #eef5ff; box-shadow: 0 0 0 2px #0169ff26; }
  .exchangeAction.actionHighlight { box-shadow: 0 0 0 4px #0169ff26, 0 5px 18px #0169ff25; }
  .cursor { position: absolute; z-index: 2; top: var(--cursor-y); left: var(--cursor-x); width: 23px; height: 28px; pointer-events: none; filter: drop-shadow(0 3px 2px #14254430); transition: top 1s cubic-bezier(.22,1,.36,1), left 1s cubic-bezier(.22,1,.36,1); animation: cursorPulse 3s ease-in-out infinite; }
  .paused, .paused .cursor { animation-play-state: paused; }
  @keyframes cardIn { from { opacity: 0; transform: translateY(16px); } }
  @keyframes cursorPulse { 0%, 100% { scale: 1; } 50% { scale: .9; } }
  @container (max-width: 320px) {
    .fields { grid-template-columns: minmax(0, 1fr) 18px minmax(0, 1fr); gap: 5px; }
    .fieldControl { padding: 5px; gap: 3px; }
    .currency { gap: 3px; padding: 2px; margin-right: -2px; }
    .currency img { width: 15px; height: 15px; }
    .currencyText strong { font-size: 10px; }
    .currencyText small { max-width: 46px; font-size: 6px; }
    .chevron { width: 7px; margin-left: 0; }
    .swapDirection { width: 18px; height: 18px; margin-bottom: 13px; }
    .swapDirection svg { width: 13px; height: 13px; }
    .cardFooter > span { font-size: 7px; }
    .fees { font-size: 8px; }
  }
  @media (max-height: 820px) and (min-width: 761px) { .swapCard { margin-top: 48px; padding: 18px; }.quote { padding-bottom: 12px; }.fields { margin-top: 12px; }.fees { margin-top: 16px; }.cardFooter { margin-top: 13px; } }
  @media (max-width: 760px) { .swapCard { width: calc(100% - 36px); margin-top: 48px; padding: 16px; border-radius: 20px; }.quote { padding-bottom: 12px; font-size: 10px; }.fields { margin-top: 12px; }.fees { margin-top: 16px; }.cardFooter { margin-top: 13px; } }
  @media (prefers-reduced-motion: reduce) { .swapCard, .cursor { animation: none; }.cursor, .fieldControl, .currency, .fees, .exchangeAction { transition: none; } }
</style>
