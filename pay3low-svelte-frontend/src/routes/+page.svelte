<script lang="ts">
  import { onMount } from "svelte";
  import Header from "$lib/components/Header.svelte";
  import Converter from "$lib/components/Converter.svelte";
  import BelarusP2pWarning from "$lib/components/BelarusP2pWarning.svelte";
  import { generatePuzzleBackground } from "$lib/puzzle-background";
  import { localize, locale, t } from "$lib/i18n";
  let shell: HTMLDivElement;
  let showBelarusP2pWarning = false;
  let belarusP2pWarningOpen = false;
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
  <Header {showBelarusP2pWarning} onOpenBelarusP2pWarning={() => belarusP2pWarningOpen = true} />
  <main><Converter onBelarusP2pWarningChange={(show) => showBelarusP2pWarning = show} /></main>
  <footer class="siteFooter"><span>Pay3Flow</span><span>{t("Live routing infrastructure · Public market estimates", {}, $locale)}</span></footer>
</div>
<BelarusP2pWarning open={belarusP2pWarningOpen} onClose={() => belarusP2pWarningOpen = false} />
