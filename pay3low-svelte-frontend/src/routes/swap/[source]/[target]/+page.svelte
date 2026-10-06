<script lang="ts">
  import HomePage from "$lib/components/HomePage.svelte";
  import type { PageData } from "./$types";
  export let data: PageData;
  $: title = `${data.amount ? `${data.amount} ` : ""}${data.source} → ${data.target} | Pay3Flow Bridge`;
  $: description = data.amount
    ? `Explore live routes for ${data.amount} ${data.source} to ${data.target} on Pay3Flow.`
    : `Explore live routes from ${data.source} to ${data.target} on Pay3Flow.`;
</script>

<svelte:head>
  <title>{title}</title>
  <meta name="description" content={description} />
  <link rel="canonical" href={data.canonical} />
  <meta property="og:type" content="website" />
  <meta property="og:title" content={title} />
  <meta property="og:description" content={description} />
  <meta property="og:url" content={data.canonical} />
  <meta property="og:image" content={data.image} />
  <meta property="og:image:type" content="image/png" />
  <meta property="og:image:width" content="1200" />
  <meta property="og:image:height" content="630" />
  <meta name="twitter:card" content="summary_large_image" />
  <meta name="twitter:title" content={title} />
  <meta name="twitter:description" content={description} />
  <meta name="twitter:image" content={data.image} />
</svelte:head>

<HomePage />
