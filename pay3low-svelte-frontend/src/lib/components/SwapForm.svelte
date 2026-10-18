<script lang="ts">
  import { locale, t } from "$lib/i18n";
  import { assetIcon, venueIcon } from "$lib/icons";
  export let provider: string;
  export let providerName: string;
  export let amount: string;
  export let fromSymbol: string;
  export let toSymbol: string;
  export let fromNetwork: string;
  export let toNetwork: string;
  export let output: string | null | undefined = null;
  export let networkNames: Record<string, string> = {};
  export let busy = false;
  $: copy = (key: string) => t(key, {}, $locale);
</script>
<section class="execution" data-testid="route-wallet-execution" data-provider={provider} aria-label={copy("Execute with wallet")}>
    <div class="executionTitle"><strong>Pay3Flow · SWAP</strong><span>{copy("Non-custodial")}</span></div>
    <div class="provider"><img src={venueIcon(provider)} alt="" width="20" height="20" /><span>{providerName}</span></div>
    <div class="swapForm" data-testid="wallet-swap-form" data-provider={provider} aria-busy={busy}>
      <div class="assetCard">
        <label>{copy("You pay")}
          <span class="assetAmount"><input aria-label={copy("Swap amount")} readonly value={amount} /><span><img src={assetIcon(fromSymbol)} alt="" />{fromSymbol}</span></span>
        </label>
        <small>{networkNames[fromNetwork] ?? fromNetwork}</small>
      </div>
      <div class="swapArrow" aria-hidden="true">↓</div>
      <div class="assetCard">
        <small>{copy("Estimated output")}</small>
        <div class="assetAmount"><strong data-testid="swap-output">{output ?? "—"}</strong><span><img src={assetIcon(toSymbol)} alt="" />{toSymbol}</span></div>
        <small>{networkNames[toNetwork] ?? toNetwork}</small>
      </div>
      <slot />
    </div>
</section>
<style>
  .execution { min-width: 0; color: var(--color-text); display: grid; gap: 12px; margin-top: 14px; padding: 14px; border: 1px solid var(--color-border); border-radius: 16px; background: var(--color-panel); }
  .executionTitle, .provider, .choiceRow { display: flex; align-items: center; gap: 8px; }
  .executionTitle { justify-content: space-between; flex-wrap: wrap; }
  .executionTitle span { padding: 4px 8px; border-radius: 999px; background: var(--color-accent-soft); color: var(--color-accent-text); font-size: 12px; font-weight: 800; text-transform: uppercase; }
  fieldset { display: grid; gap: 9px; margin: 0; padding: 0; border: 0; }
  legend, small { color: var(--color-text-soft); font-size: 12px; }
  button, input { min-height: 40px; border: 1px solid var(--color-border); border-radius: 11px; font: inherit; }
  button { padding: 0 12px; cursor: pointer; background: transparent; color: inherit; font-weight: 750; }
  button:disabled { cursor: not-allowed; opacity: .55; }
  .primary { width: 100%; border-color: var(--color-primary); background: var(--color-primary); color: var(--color-text-invert); }
  .secondary { width: fit-content; }
  .choiceRow { flex-wrap: wrap; }
  .choiceRow button { flex: 1 1 130px; }
  .choiceRow button.active { border-color: var(--color-accent-text); border-width: var(--border-highlight-width); background: var(--color-accent-soft); }
  input { width: 100%; padding: 0 11px; background: transparent; color: inherit; }
  .swapForm { display: grid; gap: 12px; }
  .assetCard { display: grid; gap: 9px; padding: 16px; border: 1px solid var(--color-border); border-radius: 14px; background: var(--color-panel-soft); }
  .assetCard label { display: grid; gap: 9px; color: var(--color-text-soft); font-size: 12px; }
  .assetAmount { color: var(--color-text); }
  .provider { color: var(--color-text-soft); font-size: 12px; }
  .provider img { flex-shrink: 0; }
  .assetAmount { display: flex; align-items: center; justify-content: space-between; gap: 16px; min-width: 0; }
  .assetAmount input, .assetAmount strong { min-width: 0; width: 100%; padding: 0; border: 0; font-size: clamp(22px, 4vw, 30px); font-weight: 750; overflow-wrap: anywhere; }
  .assetAmount span { display: flex; align-items: center; gap: 8px; font-weight: 750; }
  .assetAmount img { width: 28px; height: 28px; }
  .swapArrow { text-align: center; color: var(--color-text-soft); line-height: 1; }
  .review { display: grid; gap: 7px; padding: 10px; border-radius: 12px; background: rgba(127,127,127,.08); }
  .review span { display: flex; justify-content: space-between; gap: 12px; }
  .review strong { overflow-wrap: anywhere; text-align: right; font-size: 12px; }
  p { margin: 0; line-height: 1.45; }
  .muted, .safety, .address { color: var(--color-text-soft); font-size: 12px; }
  .ready { color: var(--color-accent-text); font-size: 12px; }
  .success { color: var(--color-good); font-weight: 700; }
  .errorText { color: var(--color-danger); font-size: 12px; }
  .safety { padding-top: 4px; border-top: 1px solid var(--color-border); }
  .recipientControls { display: grid; gap: 12px; }

  @media (max-width: 980px), (pointer: coarse) { button, input { min-height: 44px; } input { font-size: 16px; } }
</style>
