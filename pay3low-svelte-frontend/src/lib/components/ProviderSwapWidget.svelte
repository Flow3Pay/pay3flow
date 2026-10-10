<script lang="ts">
  import { assetIcon, networkIcon, venueIcon } from "$lib/icons";
  import { locale, t } from "$lib/i18n";

  export let provider: "cow-swap" | "symbiosis";
  export let from: string;
  export let to: string;
  export let input: string;
  export let output: string;
  export let sourceNetwork = "";
  export let targetNetwork = "";
  export let illustration = false;
  export let kind: string | undefined = undefined;
  $: cow = provider === "cow-swap";
  $: copy = (key: string) => t(key, {}, $locale);
  $: fields = [
    { side: "send", symbol: from, amount: input, network: sourceNetwork },
    { side: "receive", symbol: to, amount: output, network: targetNetwork },
  ];
</script>

<!-- The route fixes both assets and networks. These badges deliberately have no picker. -->
<div class="providerWidget" class:cow class:symbiosis={!cow} class:illustration data-provider={provider} data-testid={`${provider}-widget`}>
  <div class="widgetHeader">
    <strong class="widgetTitle">{cow ? "Swap" : "Swap & Bridge"}</strong>
    <span class="venue">{#if cow}<span class="cowLogo" aria-hidden="true"></span>{:else}<img src={venueIcon(provider)} alt="" />{/if}{cow ? "CoW Swap" : "symbiosis"}</span>
  </div>
  <div class="fields">
    {#each fields as field, index}
      <div class="field" role="group" aria-label={copy(index === 0 ? "You pay" : "Estimated output")} class:amountHighlight={kind === "verify" && index === 0 || kind === "review" && index === 1} class:routeHighlight={kind === "open"} data-side={field.side}>
        {#if !cow}<small class="networkLabel">{copy(index === 0 ? "From" : "To")}: <span data-testid={`${provider}-${field.side}-network`}>{field.network}</span></small>{/if}
        <div class="fieldValue">
          <span class="tokenIcon"><img src={assetIcon(field.symbol)} alt="" />{#if field.network}<img class="networkIcon" src={networkIcon(field.network)} alt="" />{/if}</span>
          {#if !illustration && index === 0}
            <input aria-label={copy("Swap amount")} readonly value={field.amount} />
          {:else}
            <strong class="amount" data-testid={`${provider}-${field.side}-amount`}>{field.amount}</strong>
          {/if}
          <span class="tokenBadge"><span class="cowTokenIcon"><img src={assetIcon(field.symbol)} alt="" />{#if field.network}<img class="networkIcon" src={networkIcon(field.network)} alt="" />{/if}</span><strong>{field.symbol}</strong></span>
        </div>
        {#if cow}<small class="networkLabel" data-testid={`${provider}-${field.side}-network`}>{field.network}</small>{/if}
      </div>
    {/each}
    <span class="directionArrow" aria-hidden="true">↓</span>
  </div>
  {#if !cow}
    <div class="routeSummary" class:quoteHighlight={kind === "review"}>
      <small>{copy("Selected route")}</small>
      <div><span><img src={networkIcon(sourceNetwork)} alt="" />{sourceNetwork}</span><i aria-hidden="true"></i><span><img src={networkIcon(targetNetwork)} alt="" />{targetNetwork}</span></div>
    </div>
  {/if}
  <div class="widgetDetails"><slot /></div>
  <div class="widgetAction"><slot name="action" /></div>
</div>

<style>
  .providerWidget { --widget-bg: #fff; --widget-field: #f2f2f2; --widget-text: #00234e; --widget-muted: #506784; --widget-border: transparent; --widget-badge: #fff; --widget-accent: #004591; --widget-action-text: #65d9ff; --widget-highlight: #0073bb; box-sizing: border-box; display: grid; gap: 10px; width: 100%; min-width: 0; padding: 10px; border-radius: 24px; background: var(--widget-bg); color: var(--widget-text); font-family: Inter, Arial, sans-serif; }
  .symbiosis { --widget-field: #f8f9fb; --widget-text: #111218; --widget-muted: #929bad; --widget-border: #e9eaee; --widget-accent: #111218; --widget-action-text: #fff; --widget-highlight: #3a9825; padding: 20px; gap: 16px; box-shadow: 0 2px 4px #11121808; }
  :global(html[data-theme="dark"]) .cow { --widget-bg: #181834; --widget-field: #0d0e21; --widget-text: #dee3e6; --widget-muted: #a6a9b6; --widget-badge: #181834; --widget-accent: #65d9ff; --widget-action-text: #0d0e21; --widget-highlight: #65d9ff; }
  :global(html[data-theme="dark"]) .symbiosis { --widget-bg: #222430; --widget-field: #272937; --widget-text: #fff; --widget-muted: #9a9fac; --widget-border: #353643; --widget-badge: #272937; --widget-accent: #a2ff3d; --widget-action-text: #111218; --widget-highlight: #a2ff3d; }
  .widgetHeader { display: flex; align-items: center; justify-content: space-between; gap: 10px; min-height: 28px; padding: 0 8px; }
  .widgetTitle { font-size: 16px; font-weight: 600; }
  .cow .widgetTitle { padding: 4px 10px; border-radius: 16px; background: var(--widget-field); font-size: 14px; font-weight: 500; }
  .venue { display: flex; align-items: center; gap: 5px; font-size: 11px; font-weight: 600; }
  .venue img, .cowLogo { width: 19px; height: 19px; }
  .cowLogo { background: var(--widget-accent); mask: url("/icons/venues/cow-swap-favicon.svg") center / contain no-repeat; }
  .symbiosis .widgetHeader { padding: 0; }
  .fields { position: relative; display: grid; gap: 10px; min-width: 0; }
  .field { min-width: 0; border: 1px solid var(--widget-border); border-radius: 17px; background: var(--widget-field); padding: 22px 16px 14px; }
  .fieldValue { display: flex; align-items: center; gap: 10px; min-width: 0; }
  .amount, input { flex: 1; width: 0; min-width: 0; margin: 0; padding: 0; border: 0; background: transparent; color: inherit; font: inherit; font-size: 30px; font-weight: 500; font-variant-numeric: tabular-nums; overflow-wrap: anywhere; }
  input { outline-offset: 4px; }
  .networkLabel { display: block; margin-top: 12px; font-size: 11px; color: var(--widget-muted); overflow-wrap: anywhere; }
  .tokenBadge { display: flex; align-items: center; gap: 6px; flex: 0 0 auto; padding: 5px 9px 5px 5px; background: var(--widget-badge); border-radius: 24px; box-shadow: 0 3px 8px #00234e0a; }
  .tokenBadge strong { font-size: 17px; font-weight: 500; }
  .cowTokenIcon, .tokenIcon { position: relative; display: block; flex: 0 0 28px; width: 28px; height: 28px; }
  .cowTokenIcon > img, .tokenIcon > img { width: 28px; height: 28px; border-radius: 50%; }
  .cowTokenIcon > .networkIcon, .tokenIcon > .networkIcon { position: absolute; right: -2px; bottom: -2px; width: 12px; height: 12px; border: 1px solid var(--widget-badge); }
  .cow .tokenIcon { display: none; }
  .directionArrow { position: absolute; left: 50%; top: 50%; transform: translate(-50%, -50%); display: grid; place-items: center; width: 28px; height: 28px; border: 3px solid var(--widget-bg); border-radius: 11px; background: var(--widget-field); color: var(--widget-text); font-size: 22px; }
  .symbiosis .fields { gap: 38px; }
  .symbiosis .field { padding: 0; border: 0; border-radius: 0; background: transparent; }
  .symbiosis .networkLabel { margin: 0 0 10px; }
  .symbiosis .fieldValue { padding: 8px 12px; border: 1px solid var(--widget-border); border-radius: 16px; background: var(--widget-field); }
  .symbiosis .amount, .symbiosis input { font-size: 18px; }
  .symbiosis .tokenBadge { padding: 0; background: transparent; box-shadow: none; }
  .symbiosis .tokenBadge strong { font-size: 12px; color: var(--widget-muted); }
  .symbiosis .cowTokenIcon { display: none; }
  .symbiosis .directionArrow { top: calc(50% + 9px); width: 26px; height: 26px; background: var(--widget-accent); color: var(--widget-action-text); font-size: 16px; }
  .routeSummary { padding: 12px; border-radius: 16px; background: var(--widget-field); }
  .routeSummary > small { font-size: 12px; }
  .routeSummary > div { display: flex; align-items: center; gap: 10px; margin-top: 12px; font-size: 10px; color: var(--widget-muted); }
  .routeSummary span { display: flex; align-items: center; gap: 4px; min-width: 0; overflow-wrap: anywhere; }
  .routeSummary img { width: 18px; height: 18px; border-radius: 50%; }
  .routeSummary i { flex: 1; min-width: 10px; border-top: 1px dashed var(--widget-muted); }
  .widgetDetails { min-width: 0; color: var(--widget-muted); }
  .widgetDetails:empty, .widgetAction:empty { display: none; }
  .widgetAction :global(button), .widgetAction :global(.exchangeAction) { box-sizing: border-box; display: grid; place-items: center; width: 100%; min-height: 52px; padding: 10px; border: 0; border-radius: 16px; background: var(--widget-accent); color: var(--widget-action-text); font-size: 17px; font-weight: 600; }
  .symbiosis .widgetAction :global(button), .symbiosis .widgetAction :global(.exchangeAction) { min-height: 44px; border-radius: 12px; font-size: 14px; }
  .amountHighlight, .routeHighlight { outline: 2px solid var(--widget-highlight); outline-offset: -2px; }
  .symbiosis .amountHighlight, .symbiosis .routeHighlight { outline: 0; }
  .symbiosis .amountHighlight .fieldValue, .symbiosis .routeHighlight .fieldValue { outline: 2px solid var(--widget-highlight); outline-offset: -2px; }
  .quoteHighlight { color: var(--widget-text); }
  .illustration { height: 100%; gap: 8px; padding: 10px; border-radius: 20px; }
  .illustration .widgetHeader { min-height: 22px; }
  .illustration .widgetTitle { font-size: 12px; }
  .illustration .venue { font-size: 9px; }
  .illustration .venue img, .illustration .cowLogo { width: 15px; height: 15px; }
  .illustration .fields { align-self: stretch; }
  .illustration.cow { grid-template-rows: auto 1fr auto auto; }
  .illustration .field { padding: 12px; }
  .illustration .amount { font-size: clamp(16px, 6cqw, 25px); }
  .illustration .tokenBadge strong { font-size: 13px; }
  .illustration .networkLabel { font-size: 9px; margin-top: 6px; }
  .illustration .widgetAction :global(.exchangeAction) { min-height: 36px; font-size: 12px; padding: 7px; }
  .illustration.symbiosis { grid-template-rows: auto 1fr auto auto auto; padding: 14px; gap: 8px; }
  .illustration.symbiosis .fields { gap: 22px; }
  .illustration.symbiosis .field { padding: 0; }
  .illustration.symbiosis .networkLabel { margin: 0 0 5px; }
  .illustration.symbiosis .fieldValue { padding: 7px 9px; }
  .illustration.symbiosis .amount { font-size: 16px; }
  .illustration.symbiosis .tokenBadge strong { font-size: 10px; }
  .illustration .routeSummary { padding: 8px 10px; border-radius: 12px; }
  .illustration .routeSummary > small { font-size: 9px; }
  .illustration .routeSummary > div { margin-top: 5px; font-size: 8px; }
  @container (max-width: 280px) { .illustration .venue { font-size: 8px; }.illustration .tokenBadge strong { font-size: 11px; }.illustration .field { padding: 9px; } }
  @container (max-height: 280px) {
    .illustration { gap: 5px; padding: 10px; }
    .illustration .widgetHeader { min-height: 18px; }
    .illustration .field { padding: 9px; }
    .illustration .amount { font-size: 20px; }
    .illustration .networkLabel { margin-top: 4px; font-size: 8px; }
    .illustration .cowTokenIcon, .illustration .tokenIcon { flex-basis: 22px; width: 22px; height: 22px; }
    .illustration .cowTokenIcon > img, .illustration .tokenIcon > img { width: 22px; height: 22px; }
    .illustration .cowTokenIcon > .networkIcon, .illustration .tokenIcon > .networkIcon { width: 10px; height: 10px; }
    .illustration .widgetAction :global(.exchangeAction) { min-height: 28px; padding: 5px; font-size: 10px; }
    .illustration.symbiosis { padding: 10px; gap: 5px; }
    .illustration.symbiosis .fields { gap: 18px; }
    .illustration.symbiosis .networkLabel { margin: 0 0 3px; }
    .illustration.symbiosis .fieldValue { padding: 5px 8px; }
    .illustration.symbiosis .amount { font-size: 14px; }
    .illustration .routeSummary { padding: 5px 8px; }
    .illustration .routeSummary > div { margin-top: 3px; }
    .illustration .routeSummary img { width: 14px; height: 14px; }
  }
</style>
