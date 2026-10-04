<script lang="ts">
  import { getContext } from "svelte";
  import type { Readable } from "svelte/store";

  type Point = { time: number; count: number };
  const { data, xGet, yGet, height, width, yScale } = getContext<{
    data: Readable<Point[]>;
    xGet: Readable<(point: Point) => number>;
    yGet: Readable<(point: Point) => number>;
    height: Readable<number>;
    width: Readable<number>;
    yScale: Readable<(value: number) => number>;
  }>("LayerCake");

  $: line = $data.length ? `M${$data.map((point) => `${$xGet(point)},${$yGet(point)}`).join("L")}` : "";
  $: area = line ? `${line}L${$width},${$height}L0,${$height}Z` : "";
  const grid = [0, 0.5, 1];
</script>

{#each grid as fraction}
  <line class="grid" x1="0" x2={$width} y1={$yScale(Math.max(...$data.map((point) => point.count), 1) * fraction)} y2={$yScale(Math.max(...$data.map((point) => point.count), 1) * fraction)} />
{/each}
<path class="area" d={area} />
<path d={line} />

<style>
  .grid { stroke: #dce2d6; stroke-dasharray: 3 4; stroke-width: 1; }
  .area { fill: var(--color-panel-soft); }
  path:not(.area) { fill: none; stroke: #729b17; stroke-width: 2.5; stroke-linecap: round; stroke-linejoin: round; vector-effect: non-scaling-stroke; }
  :global(html[data-theme="dark"]) .grid { stroke: #383838; }
  :global(html[data-theme="dark"]) path:not(.area) { stroke: #b9e834; }
</style>
