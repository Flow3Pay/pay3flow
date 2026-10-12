<script module lang="ts">
  import type { TrendItem } from "$lib/trends/model";
  export type TrendBrickData = {
    enterDelay: number; item: TrendItem; active: boolean; metricLabel: string; value: string;
    networkLabel: string; providerLabel: string;
    showHover: (item: TrendItem, anchor: HTMLElement) => void;
    showDetail: (item: TrendItem, anchor: HTMLElement) => void;
    scheduleClose: () => void;
  };
</script>

<script lang="ts">
  import type { Node, NodeProps } from "@xyflow/svelte";
  import WorkflowControl from "$lib/components/WorkflowControl.svelte";
  import { trendSwapHref } from "$lib/trends/model";
  let { data }: NodeProps<Node<TrendBrickData>> = $props();
  let entering = $state(true);
</script>

<div class="brickEntry" class:entering style:animation-delay={`${data.enterDelay}ms`} onanimationend={() => entering = false}>
<a href={trendSwapHref(data.item)} class="brick nodrag nopan" class:active={data.active}
  data-trend-id={data.item.id} data-parts={2 + Number(Boolean(data.item.provider))}
  aria-label={`${data.item.asset} · ${data.networkLabel}${data.item.provider ? ` · ${data.providerLabel}` : ""} · ${data.metricLabel}: ${data.value}`}
  aria-expanded={data.active} aria-controls={data.active ? "trend-details" : undefined}
  onpointerenter={(event) => { if (event.pointerType === "mouse") { entering = false; data.showHover(data.item, event.currentTarget); } }}
  onpointerleave={data.scheduleClose} onfocus={(event) => { entering = false; data.showDetail(data.item, event.currentTarget); }} onblur={data.scheduleClose}>
  <WorkflowControl asset={data.item.asset} network={data.item.network} networkLabel={data.networkLabel} provider={data.item.provider} providerLabel={data.providerLabel} />
</a>
</div>

<style>
  .brickEntry { display: inline-flex; }
  .brickEntry.entering { animation: layBrick .42s cubic-bezier(.22, 1, .36, 1) both; }
  @keyframes layBrick { from { opacity: 0; transform: translateY(49px) scale(.97); } to { opacity: 1; transform: translateY(0) scale(1); } }
  .brick { display: inline-flex; align-items: stretch; width: max-content; height: 41px; padding: 0; border: 0; border-radius: 5px; background: transparent; box-shadow: 0 4px 0 #cdd3c5; transition: transform .16s, box-shadow .16s; }
  .brick:hover, .brick.active { transform: scale(1.045); box-shadow: 2px 5px 0 #bfc7b6; }
  :global(html[data-theme="dark"]) .brick { box-shadow: 0 4px 0 #202020; }
  :global(html[data-theme="dark"]) .brick:hover, :global(html[data-theme="dark"]) .brick.active { box-shadow: 2px 5px 0 #171717; }
  @media (prefers-reduced-motion: reduce) { .brick { transition: none; } .brickEntry.entering { animation: none; } }
</style>
