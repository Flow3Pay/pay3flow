<script lang="ts">
  import { onMount } from "svelte";
  import Header from "$lib/components/Header.svelte";
  import Converter from "$lib/components/Converter.svelte";
  import { generatePuzzleBackground } from "$lib/puzzle-background";
  import { localize, locale, t } from "$lib/i18n";
  let shell: HTMLDivElement;
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

<svelte:head><title>Pay3Flow</title></svelte:head>

<div class="appShell" bind:this={shell} use:localize>
  <Header />
  <main><Converter /></main>
  <footer class="siteFooter"><span>Pay3Flow</span><span>{t("Live routing infrastructure · Public market estimates", {}, $locale)}</span></footer>
</div>
