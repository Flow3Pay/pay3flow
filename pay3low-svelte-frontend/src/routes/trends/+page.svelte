<script lang="ts">
  import Header from "$lib/components/Header.svelte";
  import TrendsWall from "$lib/components/TrendsWall.svelte";
  import { onMount } from "svelte";
  import { localize, setLocale } from "$lib/i18n";
  import type { PageData } from "./$types";
  export let data: PageData;
  onMount(() => { if (data.requestedLanguage) setLocale(data.requestedLanguage); });
</script>

<svelte:head>
  <title>{data.title}</title>
  <meta name="description" content={data.description} />
  <link rel="canonical" href={data.canonical} />
  <meta property="og:type" content="website" />
  <meta property="og:site_name" content="Pay3Flow" />
  <meta property="og:url" content={data.shareUrl} />
  <meta property="og:title" content={data.title} />
  <meta property="og:description" content={data.description} />
  <meta property="og:image" content={data.imageUrl} />
  <meta property="og:image:width" content="1200" />
  <meta property="og:image:height" content="630" />
  <meta property="og:image:type" content="image/png" />
  <meta property="og:image:alt" content={data.imageAlt} />
  <meta name="twitter:card" content="summary_large_image" />
  <meta name="twitter:title" content={data.title} />
  <meta name="twitter:description" content={data.description} />
  <meta name="twitter:image" content={data.imageUrl} />
  <meta name="twitter:image:alt" content={data.imageAlt} />
</svelte:head>

<div class="appShell" use:localize>
  <Header activePage="trends" shareUrl="/trends" />
  <main><TrendsWall /></main>
</div>

<style>
  .appShell { height: 100dvh; display: flex; flex-direction: column; overflow: hidden; background: var(--color-paper); }
  main { flex: 1; min-height: 0; }
  .appShell :global(header) { flex-shrink: 0; }
</style>
