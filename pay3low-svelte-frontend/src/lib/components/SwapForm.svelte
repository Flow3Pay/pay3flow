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
    <div class="executionTitle"><strong>Pay3Flow · SWAP</strong></div>
    <div class="provider"><img src={venueIcon(provider)} alt="" width="20" height="20" /><span>{providerName}</span></div>
    <div class="swapForm" data-testid="wallet-swap-form" data-provider={provider} aria-busy={busy}>
      <div class="assetCard">
        <label>{copy("You pay")}
          <span class="assetAmount"><input aria-label={copy("Swap amount")} readonly value={amount} /><span><img src={assetIcon(fromSymbol)} alt="" />{fromSymbol}</span></span>
        </label>
        <small>{networkNames[fromNetwork] ?? fromNetwork}</small>
      </div>
      <div class="swapArrow">
        <button type="button" class="bridgeIcon" aria-label={copy("Swap sender and recipient")} title={copy("Swap sender and recipient")} disabled>
          <svg width="22" height="22" viewBox="5.5 5.7 13 13" aria-hidden="true" focusable="false"><path d="M17.85 14.1C17.81 14.19 17.76 14.27 17.69 14.34L14.65 17.38C14.5 17.53 14.31 17.6 14.12 17.6C13.93 17.6 13.74 17.53 13.59 17.38C13.3 17.09 13.3 16.61 13.59 16.32L15.35 14.56H6.85C6.44 14.56 6.1 14.22 6.1 13.81C6.1 13.4 6.44 13.06 6.85 13.06H17.16C17.26 13.06 17.35 13.08 17.45 13.12C17.63 13.2 17.78 13.34 17.86 13.53C17.92 13.71 17.92 13.92 17.85 14.1ZM17.15 10.93H6.85C6.75 10.93 6.65 10.91 6.56 10.87C6.38 10.79 6.23 10.65 6.15 10.46C6.07 10.28 6.07 10.07 6.15 9.89C6.19 9.8 6.24 9.72 6.31 9.65L9.35 6.61C9.64 6.32 10.12 6.32 10.41 6.61C10.7 6.9 10.7 7.38 10.41 7.67L8.66 9.43H17.16C17.57 9.43 17.91 9.77 17.91 10.18C17.91 10.59 17.57 10.93 17.15 10.93Z"></path></svg>
        </button>
      </div>
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
  .swapArrow { display: flex; justify-content: center; color: var(--color-text-soft); line-height: 1; }
  .bridgeIcon { display: grid; width: 28px; height: 28px; place-items: center; padding: 0; border: 0; border-radius: 7px; background: var(--exchange-card-bg); color: var(--color-text-soft); cursor: default; box-shadow: none; }
  .bridgeIcon:disabled { opacity: 1; cursor: default; }
  :global(html[data-theme="dark"]) .bridgeIcon { color: #fff; }
  .bridgeIcon svg { width: 22px; height: 22px; fill: currentColor; }
  .review { display: grid; gap: 7px; padding: 10px; border-radius: 12px; background: rgba(127,127,127,.08); }
  .review span { display: flex; justify-content: space-between; gap: 12px; }
  .review strong { overflow-wrap: anywhere; text-align: right; font-size: 12px; }
  p { margin: 0; line-height: 1.45; }
  .muted, .safety, .address { color: var(--color-text-soft); font-size: 12px; }
  .ready { color: var(--color-accent-text); font-size: 12px; }
  .success { color: var(--color-good-ink); font-weight: 700; }
  .errorText { color: var(--color-danger); font-size: 12px; }
  .safety { padding-top: 4px; border-top: 1px solid var(--color-border); }
  .recipientControls { display: grid; gap: 12px; }

  @media (max-width: 980px), (pointer: coarse) { button, input { min-height: 44px; } input { font-size: 16px; } }
</style>
