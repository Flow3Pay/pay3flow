<script lang="ts">
  import { onMount } from "svelte";
  import Header from "$lib/components/Header.svelte";
  import Converter from "$lib/components/Converter.svelte";
  import HomeOverview from "$lib/components/HomeOverview.svelte";
  import { COMMUNITY_URL, PROJECT_URL, SITE_URL, homeContent } from "$lib/home-content";
  import BelarusP2pWarning from "$lib/components/BelarusP2pWarning.svelte";
  import { generatePuzzleBackground } from "$lib/puzzle-background";
  import { localize, locale, t } from "$lib/i18n";
  let shell: HTMLDivElement;
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
  <title>{copy.title}</title>
  <meta name="description" content={copy.description} />
  <link rel="canonical" href={SITE_URL} />
  <meta property="og:type" content="website" />
  <meta property="og:site_name" content="Pay3Flow" />
  <meta property="og:url" content={SITE_URL} />
  <meta property="og:title" content={copy.title} />
  <meta property="og:description" content={copy.description} />
  {@html `<script type="application/ld+json">${website}</script>`}
</svelte:head>

<div class="appShell" bind:this={shell} use:localize>
  <Header />
  <main>
    <Converter onBelarusP2pWarningChange={(show) => showBelarusP2pWarning = show} onOpenBelarusP2pWarning={() => belarusP2pWarningOpen = true} />
    <HomeOverview />
  </main>
  <footer class="siteFooter"><span>Pay3Flow</span><span>{t("Live routing infrastructure · Public market estimates", {}, $locale)}</span><a href="/terms">{t("Usage policy", {}, $locale)}</a></footer>
</div>
<BelarusP2pWarning open={belarusP2pWarningOpen} onClose={() => belarusP2pWarningOpen = false} />
