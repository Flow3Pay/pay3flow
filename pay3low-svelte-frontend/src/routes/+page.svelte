<script lang="ts">
  import { onMount } from "svelte";
  import type { PaymentMethod } from "$lib/payment-methods";
  import type { ProviderDefinition } from "$lib/exchange";
  import type { ExchangeShareState } from "$lib/exchange-share";
  import Header from "$lib/components/Header.svelte";
  import type OtcWorkspaceType from "$lib/components/otc/OtcWorkspace.svelte";
  import Converter from "$lib/components/Converter.svelte";
  import HomeOverview from "$lib/components/HomeOverview.svelte";
  import { COMMUNITY_URL, PROJECT_URL, SITE_URL, homeContent } from "$lib/home-content";
  import BelarusP2pWarning from "$lib/components/BelarusP2pWarning.svelte";
  import { generatePuzzleBackground } from "$lib/puzzle-background";
  import { localize, locale, t } from "$lib/i18n";
  let guideActive = false;
  let otcActive = false;
  let OtcWorkspace: typeof OtcWorkspaceType | null = null;
  let swapHref = "/#/swap";
  let otcShareUrl = "/#/otc";
  function rememberSwap() {
    if (/^#\/(?:swap|guide)(?:[/?]|$)/.test(window.location.hash)) swapHref = `/${window.location.hash.replace(/^#\/guide/, "#/swap")}`;
    else if (!otcActive) swapHref = "/#/swap";
  }
  async function syncProductPage() {
    const nextOtc = /^#\/otc(?:[/?]|$)/.test(window.location.hash);
    if (!nextOtc && /^#\/(?:swap|guide)(?:[/?]|$)/.test(window.location.hash)) rememberSwap();
    otcActive = nextOtc;
    if (nextOtc) {
      guideActive = false;
      shareState = null;
      otcShareUrl = `/${window.location.hash}`;
      OtcWorkspace ??= (await import("$lib/components/otc/OtcWorkspace.svelte")).default;
    }
  }
  onMount(() => {
    void syncProductPage();
    window.addEventListener("hashchange", syncProductPage);
    window.addEventListener("popstate", syncProductPage);
    return () => { window.removeEventListener("hashchange", syncProductPage); window.removeEventListener("popstate", syncProductPage); };
  });
  let shareState: ExchangeShareState | null = null;
  let shell: HTMLDivElement;
  let paymentMethods: PaymentMethod[] = [];
  let providerCatalog: ProviderDefinition[] = [];
  let showBelarusP2pWarning = false;
  let belarusP2pWarningOpen = false;
  $: copy = homeContent[$locale];
  $: website = JSON.stringify({
    "@context": "https://schema.org",
    "@type": "WebSite",
    name: "Pay3Flow",
    url: SITE_URL,
    description: copy.description,
    inLanguage: $locale,
    sameAs: [PROJECT_URL, COMMUNITY_URL],
  }).replace(/</g, "\\u003c");
  onMount(() => shell.style.setProperty("--puzzle-pattern", generatePuzzleBackground()));
  onMount(() => {
    let zoomTimer: number | undefined;
    const viewport = window.visualViewport;
    const markViewportChange = () => {
      document.documentElement.dataset.zooming = "true";
      if (zoomTimer) window.clearTimeout(zoomTimer);
      zoomTimer = window.setTimeout(() => {
        delete document.documentElement.dataset.zooming;
      }, 180);
    };

    window.addEventListener("resize", markViewportChange, { passive: true });
    viewport?.addEventListener("resize", markViewportChange, { passive: true });
    return () => {
      window.removeEventListener("resize", markViewportChange);
      viewport?.removeEventListener("resize", markViewportChange);
      if (zoomTimer) window.clearTimeout(zoomTimer);
      delete document.documentElement.dataset.zooming;
    };
  });
</script>

<svelte:head>
  <title>{otcActive ? "Pay3Flow — OTC" : copy.title}</title>
  <meta name="description" content={copy.description} />
  <link rel="canonical" href={SITE_URL} />
  <meta property="og:type" content="website" />
  <meta property="og:site_name" content="Pay3Flow" />
  <meta property="og:url" content={SITE_URL} />
  <meta property="og:title" content={copy.title} />
  <meta property="og:description" content={copy.description} />
  {@html `<script type="application/ld+json">${website}</script>`}
</svelte:head>

<div class="appShell" class:guideActive bind:this={shell} use:localize>
  <Header shareState={guideActive || otcActive ? null : shareState} activePage={otcActive ? "otc" : "swap"} {swapHref} onOtcNavigate={rememberSwap} shareUrl={otcActive ? otcShareUrl : null} />
  <main>
    {#if otcActive}
      {#if OtcWorkspace}<svelte:component this={OtcWorkspace} />{:else}<div class="otcLoading" aria-busy="true">OTC<span>…</span></div>{/if}
    {:else}
    <Converter onShareStateChange={(state) => shareState = state} onGuideChange={(active) => guideActive = active} onPaymentMethodsLoaded={(items) => paymentMethods = items} onProvidersLoaded={(items) => providerCatalog = items} onBelarusP2pWarningChange={(show) => showBelarusP2pWarning = show} onOpenBelarusP2pWarning={() => belarusP2pWarningOpen = true} />
    {#if !guideActive}<HomeOverview {paymentMethods} {providerCatalog} />{/if}
    {/if}
  </main>
  {#if !guideActive && !otcActive}<footer class="siteFooter"><span>Pay3Flow</span><span>{t("Live routing infrastructure · Public market estimates", {}, $locale)}</span><a href="/terms">{t("Usage policy", {}, $locale)}</a></footer>{/if}
</div>
<BelarusP2pWarning open={!otcActive && belarusP2pWarningOpen} onClose={() => belarusP2pWarningOpen = false} />

<style>
  .otcLoading { width: min(var(--layout-width), calc(100% - 2 * var(--page-gutter))); min-height: 70vh; margin: 40px auto; font-family: var(--font-mono); color: var(--color-text-soft); }
  .otcLoading span { margin-left: 8px; }
</style>
