<script lang="ts">
  import type { Trade } from "$lib/otc/service";
  import type { OfferHistoryPoint } from "$lib/otc/public-offers";
  import OtcOfferChart from "./OtcOfferChart.svelte";
  import OtcPrivateQuoteChart from "./OtcPrivateQuoteChart.svelte";
  export let publicOffers: OfferHistoryPoint[] | null = null;
  export let trades: Trade[] = [];
</script>

{#if publicOffers !== null}
  <OtcOfferChart {publicOffers} />
{:else}
  <OtcPrivateQuoteChart {trades} />
{/if}
