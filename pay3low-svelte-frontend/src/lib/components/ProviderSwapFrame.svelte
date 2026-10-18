<script lang="ts">
  import { onMount } from "svelte";
  import type { RouteCandidate } from "$lib/exchange";
  import type { TutorialStep } from "$lib/route-tutorial";
  import { canEmbedSwapApp, swapAppUrl, symbiosisAppUrl } from "$lib/swap-embed";
  import { locale, t } from "$lib/i18n";

  export let route: RouteCandidate;
  export let step: TutorialStep;
  export let providerUrl: string | null | undefined = undefined;
  export let networkNames: Record<string, string> = {};
  let src = swapAppUrl(step, providerUrl);
  let selecting = step.provider === "symbiosis";
  let selectionFailed = false;
  $: embeddable = canEmbedSwapApp(step.provider);
  $: copy = (key: string, params: Record<string, string | number> = {}) => t(key, params, $locale);

  onMount(() => {
    const controller = new AbortController();
    if (step.provider === "symbiosis") {
      void symbiosisAppUrl(route, step, networkNames, controller.signal)
        .then(url => { src = url; })
        .catch(() => { if (!controller.signal.aborted) selectionFailed = true; })
        .finally(() => { if (!controller.signal.aborted) selecting = false; });
    }
    return () => controller.abort();
  });
</script>

<section class="providerFrame" data-testid="provider-swap-frame" data-provider={step.provider} aria-label={step.venue}>
  <div class="pair"><strong>{step.from} → {step.to}</strong><span>{step.network}{#if step.targetNetwork && step.targetNetwork !== step.network} → {step.targetNetwork}{/if}</span></div>
  {#if selecting}<p role="status">{copy("Preparing your swap…")}</p>{/if}
  {#if selectionFailed}<p role="status">{copy("Select the tokens and networks shown above in the swap form.")}</p>{/if}
  {#if src && !selecting && embeddable}
    <iframe src={src} title={step.venue} sandbox="allow-scripts allow-same-origin allow-forms allow-popups allow-popups-to-escape-sandbox" referrerpolicy="strict-origin-when-cross-origin" allow="clipboard-read; clipboard-write"></iframe>
  {:else if !embeddable}
    <div class="quote" data-testid="swap-quote">
      <div><span>{copy("You pay")}</span><strong>{step.amount ?? "—"}</strong><small>{step.network ?? ""}</small></div>
      <div><span>{copy("Estimated output")}</span><strong>{step.output ?? "—"}</strong><small>{step.targetNetwork ?? step.network ?? ""}</small></div>
      <p>{copy(step.provider === "near-intents" || step.provider === "0x" ? "This platform does not allow its swap form to be embedded. Open {venue} to complete this swap." : "Open {venue} to complete this swap.", { venue: step.venue })}</p>
    </div>
  {/if}
  {#if src}<a class:swapAction={!embeddable} href={src} target="_blank" rel="noopener noreferrer">{copy("Open {venue}", { venue: step.venue })} ↗</a>{/if}
</section>

<style>
  .providerFrame { width: 100%; margin: 24px auto 0; }
  .pair { display: flex; align-items: center; justify-content: space-between; flex-wrap: wrap; gap: 8px; margin-bottom: 12px; color: var(--color-text); }
  .pair span, p, a { font-size: 12px; color: var(--color-text-soft); }
  iframe { display: block; width: 100%; height: 840px; border: 0; border-radius: 20px; color-scheme: normal; background: transparent; }
  a { display: block; margin-top: 12px; text-align: center; }
  .quote { display: grid; gap: 10px; }
  .quote > div { display: grid; gap: 8px; padding: 18px; border: 1px solid var(--color-border); border-radius: 16px; background: var(--color-panel); }
  .quote strong { font-size: 24px; overflow-wrap: anywhere; }
  .quote span, small { font-size: 12px; color: var(--color-text-soft); }
  .swapAction { padding: 14px; border-radius: 12px; background: var(--color-primary); color: var(--color-text-invert); font-size: 14px; font-weight: 600; }
  @media (max-width: 760px) { iframe { height: 780px; border-radius: 12px; } }
</style>
