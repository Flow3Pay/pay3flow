<script lang="ts">
  import { onMount } from "svelte";
  import Header from "$lib/components/Header.svelte";
  import HomeOverview from "$lib/components/HomeOverview.svelte";
  import { SITE_URL, homeContent } from "$lib/home-content";
  import { fetchPaymentMethods, type PaymentMethod } from "$lib/payment-methods";
  import { fetchProviders, type ProviderDefinition } from "$lib/exchange";
  import { generatePuzzleBackground } from "$lib/puzzle-background";
  import { localize, locale, t } from "$lib/i18n";

  let shell: HTMLDivElement;
  let paymentMethods: PaymentMethod[] = [];
  let providerCatalog: ProviderDefinition[] = [];
  const canonical = new URL("about", SITE_URL).href;
  $: copy = homeContent[$locale];
  $: title = `Pay3Flow — ${copy.aboutTitle}`;
  onMount(() => {
    shell.style.setProperty("--puzzle-pattern", generatePuzzleBackground());
    void fetchPaymentMethods().then(items => paymentMethods = items).catch(() => {});
    void fetchProviders().then(items => providerCatalog = items).catch(() => {});
  });
</script>

<svelte:head>
  <title>{title}</title>
  <meta name="description" content={copy.about} />
  <link rel="canonical" href={canonical} />
  <meta property="og:type" content="website" />
  <meta property="og:site_name" content="Pay3Flow" />
  <meta property="og:url" content={canonical} />
  <meta property="og:title" content={title} />
  <meta property="og:description" content={copy.about} />
</svelte:head>

<div class="appShell" bind:this={shell} use:localize>
  <Header activePage="about" shareUrl="/about" />
  <main><HomeOverview {paymentMethods} {providerCatalog} /></main>
  <footer class="siteFooter"><span>Pay3Flow</span><span>{t("Live routing infrastructure · Public market estimates", {}, $locale)}</span><a href="/terms">{t("Usage policy", {}, $locale)}</a></footer>
</div>
