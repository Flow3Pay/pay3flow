<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { restoreLocale } from "$lib/i18n";
  import IconTooltip from "$lib/components/IconTooltip.svelte";
  let { children } = $props();
  onMount(restoreLocale);
</script>

{@render children()}
<IconTooltip />
