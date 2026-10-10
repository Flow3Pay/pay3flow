<script lang="ts">
  import ProviderProfile from '$lib/components/ProviderProfile.svelte';
  import type { PageData } from './$types';
  export let data: PageData;
</script>
{#key data.slug}<ProviderProfile slug={data.slug} />{/key}
