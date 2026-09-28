<script lang="ts">
  import { onMount } from "svelte";
  import Header from "$lib/components/Header.svelte";
  import Converter from "$lib/components/Converter.svelte";
  import { generatePuzzleBackground } from "$lib/puzzle-background";
  import { localize, locale, t } from "$lib/i18n";
  let shell: HTMLDivElement;
  onMount(() => shell.style.setProperty("--puzzle-pattern", generatePuzzleBackground()));
</script>

<svelte:head><title>Pay3Flow</title></svelte:head>

<div class="appShell" bind:this={shell} use:localize>
  <Header />
  <main><Converter /></main>
  <footer class="siteFooter"><span>Pay3Flow</span><span>{t("Live routing infrastructure · Public market estimates", {}, $locale)}</span></footer>
</div>
