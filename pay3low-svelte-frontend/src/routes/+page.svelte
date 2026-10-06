<script lang="ts">
  import { page } from "$app/stores";
  import { SHARE_IMAGE_VERSION } from "$lib/share";
  import HomePage from "$lib/components/HomePage.svelte";
</script>

<svelte:head>
  <title>Pay3Flow</title>
  <meta property="og:type" content="website" />
  <meta property="og:title" content="Pay3Flow Bridge" />
  <meta property="og:description" content="Find live routes for cross-border exchange." />
  <meta property="og:image" content="{$page.url.origin}/share-image.png?v={SHARE_IMAGE_VERSION}" />
  <meta property="og:image:width" content="1200" />
  <meta property="og:image:height" content="630" />
  <meta name="twitter:card" content="summary_large_image" />
</svelte:head>

<HomePage />
