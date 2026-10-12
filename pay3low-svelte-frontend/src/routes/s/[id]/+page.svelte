<script lang="ts">
  import SharedExchangePage from '$lib/components/SharedExchangePage.svelte';
  import type { PageData } from './$types';
  export let data: PageData;
</script>
<SharedExchangePage {data} />
