<script lang="ts">
  import { onMount } from 'svelte';
  import Header from './Header.svelte';
  export let data: { title: string; description: string; shareUrl: string; imageUrl: string; exchangeUrl: string; russian: boolean; otc: boolean; imageAlt: string };
  onMount(() => window.location.replace(data.exchangeUrl));
</script>
<svelte:head>
  <title>{data.title}</title>
  <meta name="description" content={data.description} />
  <link rel="canonical" href={data.shareUrl} />
  <meta property="og:type" content="website" />
  <meta property="og:site_name" content="Pay3Flow" />
  <meta property="og:title" content={data.title} />
  <meta property="og:description" content={data.description} />
  <meta property="og:url" content={data.shareUrl} />
  <meta property="og:image" content={data.imageUrl} />
  <meta property="og:image:secure_url" content={data.imageUrl} />
  <meta property="og:image:type" content="image/png" />
  <meta property="og:image:width" content="1200" />
  <meta property="og:image:height" content="630" />
  <meta property="og:image:alt" content={data.imageAlt} />
  <meta name="twitter:card" content="summary_large_image" />
  <meta name="twitter:title" content={data.title} />
  <meta name="twitter:description" content={data.description} />
  <meta name="twitter:image" content={data.imageUrl} />
</svelte:head>
<div class="appShell"><Header activePage={data.otc ? 'otc' : 'swap'} /><main><img src={data.imageUrl} alt={data.imageAlt} width="1200" height="630" /><h1>{data.title}</h1><p>{data.description}</p><a href={data.exchangeUrl}>{data.russian ? 'Открыть обмен' : 'Open exchange'} →</a></main></div>
<style>main { max-width: 900px; margin: 40px auto; padding: 24px; }img { width: 100%; height: auto; border-radius: 20px; }h1 { margin-top: 24px; font-size: 30px; }p { margin-top: 16px; color: var(--color-text-soft); }a { display: inline-flex; margin-top: 24px; padding: 14px 22px; background: var(--color-accent); color: #152016; border-radius: 12px; font-weight: 700; }</style>
