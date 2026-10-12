<script lang="ts">
  import ProviderSwapWidget from "$lib/components/ProviderSwapWidget.svelte";
  import type { RouteCandidate } from "$lib/exchange";
  import { locale, t } from "$lib/i18n";
  import { tutorialMoney, type TutorialStep } from "$lib/route-tutorial";
  import { ease, portion } from "../p2p/profile-motion";
  import { swapGuideAction } from "./frames";

  export let route: RouteCandidate;
  export let step: TutorialStep;
  export let frame = 0;
  export let playing = true;
  export let progress = 1;
  $: provider = step.provider.toLowerCase() as "cow-swap" | "symbiosis";
  $: copy = (key: string) => t(key, {}, $locale);
  $: kind = step.frames[frame]?.kind;
  $: input = step.amount ?? (step.from === route.source_currency ? tutorialMoney(route.source_amount_minor, route.source_currency, route.source_amount) : undefined);
  $: travel = ease(portion(progress, 0, .8));
  $: cursorX = kind === "open" ? 82 - 10 * travel : kind === "verify" ? 72 - 53 * travel : kind === "review" ? 19 : 19 + 55 * travel;
  $: cursorY = kind === "open" ? 10 + 16 * travel : kind === "verify" ? 26 : kind === "review" ? 26 + 25 * travel : 51 + 38 * travel;
  $: clicked = kind === "act" && progress >= .8;

  function amount(value: string | undefined, currency: string) {
    if (!value || value === "—") return "—";
    const raw = value.endsWith(` ${currency}`) ? value.slice(0, -currency.length - 1) : value;
    const numeric = Number(raw.replace(/,/g, ""));
    return Number.isFinite(numeric) ? numeric.toLocaleString($locale === "ru" ? "ru-RU" : $locale === "hy" ? "hy-AM" : "en-US", { maximumFractionDigits: 8 }) : raw;
  }
</script>

<div class="walletSwapCard" class:paused={!playing} data-testid={`${provider}-swap-card`} data-frame-kind={kind} style={`--cursor-x:${cursorX}cqw;--cursor-y:${cursorY}cqh`}>
  <ProviderSwapWidget {provider} from={step.from} to={step.to} input={amount(input, step.from)} output={amount(step.output, step.to)} sourceNetwork={step.network ?? ""} targetNetwork={step.targetNetwork ?? step.network ?? ""} illustration {kind}>
    <div class="quoteLine" class:quoteHighlight={kind === "review"}><span>{copy("Fees and slippage")}</span><span>{copy("Check on the platform")}</span></div>
    <div slot="action" class="exchangeAction" class:actionHighlight={kind === "act"} class:clicked data-testid={`${provider}-exchange-action`}>{copy(swapGuideAction(provider))}</div>
  </ProviderSwapWidget>
  <div class="cursorTrack"><svg viewBox="0 0 32 38" fill="none"><path d="M3 2L27 22L15 24L9 35L3 2Z" fill="#151c25" stroke="white" stroke-width="2.5" stroke-linejoin="round" /></svg></div>
</div>

<style>
  .walletSwapCard { position: relative; width: calc(100% - 48px); max-width: 400px; height: 330px; margin: 54px auto 100px; container-type: size; border-radius: 20px; box-shadow: 0 18px 50px #20304018; }
  .quoteLine { display: flex; align-items: center; justify-content: space-between; gap: 8px; font-size: 8px; }
  .quoteHighlight { color: var(--widget-text); }
  .actionHighlight { box-shadow: 0 0 0 3px color-mix(in srgb, var(--widget-highlight) 25%, transparent); }
  .clicked { transform: scale(.99); }
  .cursorTrack { position: absolute; z-index: 2; left: 0; top: 0; width: 22px; transform: translate3d(var(--cursor-x), var(--cursor-y), 0); pointer-events: none; }
  .cursorTrack svg { display: block; width: 22px; height: 27px; filter: drop-shadow(0 2px 2px #0004); }
  @media (max-height: 820px) and (min-width: 761px) { .walletSwapCard { margin-top: 20px; height: 250px; } }
  @media (max-width: 760px) { .walletSwapCard { width: calc(100% - 36px); margin-top: 16px; height: 248px; } }
</style>
