<script lang="ts">
  import type { RouteCandidate } from "$lib/exchange";
  import type { TutorialStep } from "$lib/route-tutorial";
  import { swapInput } from "$lib/swap-service";
  import { walletFamily } from "$lib/wallet-execution";
  import RouteExecutionPanel from "./RouteExecutionPanel.svelte";
  import SdkSwapPanel from "./SdkSwapPanel.svelte";
  export let route: RouteCandidate;
  export let step: TutorialStep;
  export let networkNames: Record<string, string> = {};
  $: input = swapInput(route, step, networkNames);
  $: native = Boolean(step.execution && route.execution && walletFamily(input.fromAsset.split("@")[1]));
</script>

<div class="unifiedSwap" data-testid="unified-swap">
  {#key input.routeId}
    {#if native}<RouteExecutionPanel {route} {networkNames} />
    {:else}<SdkSwapPanel {input} providerName={step.venue} {networkNames} initialOutput={step.output?.replace(new RegExp(` ${step.to}$`), "")} />{/if}
  {/key}
</div>

<style>
  .unifiedSwap { width: min(100%, 470px); min-width: 0; margin: 24px auto; }
</style>
