<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { restoreLocale } from "$lib/i18n";
  import { scrambleOnLoad } from "$lib/scramble-on-load";
  import IconTooltip from "$lib/components/IconTooltip.svelte";
  let { children } = $props();
  onMount(restoreLocale);
</script>

<svelte:body use:scrambleOnLoad />
{@render children()}
<IconTooltip />
