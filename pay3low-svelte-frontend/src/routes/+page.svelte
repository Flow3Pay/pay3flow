<script lang="ts">
  import { page } from "$app/stores";
  import { SHARE_IMAGE_VERSION } from "$lib/share";
  import HomePage from "$lib/components/HomePage.svelte";
  $: homeUrl = new URL("/", $page.url.origin).toString();
  $: imageUrl = new URL(`/share-image.png?v=${SHARE_IMAGE_VERSION}`, $page.url.origin).toString();
</script>

<svelte:head>
  <title>Pay3Flow</title>
  <meta name="description" content="Find live routes for cross-border exchange." />
  <link rel="canonical" href={homeUrl} />
  <meta property="og:type" content="website" />
  <meta property="og:title" content="Pay3Flow Bridge" />
  <meta property="og:description" content="Find live routes for cross-border exchange." />
  <meta property="og:url" content={homeUrl} />
  <meta property="og:image" content={imageUrl} />
  <meta property="og:image:secure_url" content={imageUrl} />
  <meta property="og:image:type" content="image/png" />
  <meta property="og:image:width" content="1200" />
  <meta property="og:image:height" content="630" />
  <meta name="twitter:card" content="summary_large_image" />
  <meta name="twitter:title" content="Pay3Flow Bridge" />
  <meta name="twitter:description" content="Find live routes for cross-border exchange." />
  <meta name="twitter:image" content={imageUrl} />
</svelte:head>

<HomePage />
