<script lang="ts">
  import { getContext } from "svelte";
  import type { Readable } from "svelte/store";

  type Point = { time: number; count: number };
  export let timeLabel: (time: number) => string;
  export let countLabel: (count: number) => string;
  export let maximum: number;
  export let label: string;
  export let activeIndex: number | null = null;
  export let onHover: (point: (Point & { x: number; width: number }) | null) => void = () => {};

  const { data, xGet, yGet, height, width, yScale } = getContext<{
    data: Readable<Point[]>;
    xGet: Readable<(point: Point) => number>;
    yGet: Readable<(point: Point) => number>;
    height: Readable<number>;
    width: Readable<number>;
    yScale: Readable<(value: number) => number>;
  }>("LayerCake");

  $: line = $data.length ? `M${$data.map((point) => `${$xGet(point)},${$yGet(point)}`).join("L")}` : "";
  const grid = [0, 0.25, 0.5, 0.75, 1];
  $: activePoint = activeIndex === null ? undefined : $data[activeIndex];
  $: onHover(activePoint ? { ...activePoint, x: $xGet(activePoint), width: $width } : null);

  function inspectPointer(event: PointerEvent) {
    const bounds = (event.currentTarget as SVGRectElement).getBoundingClientRect();
    if (!bounds.width || !$data.length) return;
    const x = (event.clientX - bounds.left) / bounds.width * $width;
    let left = 0;
    let right = $data.length - 1;
    while (left < right) {
      const middle = Math.floor((left + right) / 2);
      if ($xGet($data[middle]) < x) left = middle + 1;
      else right = middle;
    }
    activeIndex = left > 0 && Math.abs($xGet($data[left - 1]) - x) < Math.abs($xGet($data[left]) - x) ? left - 1 : left;
  }

  function inspectKeyboard(event: KeyboardEvent) {
    const current = activeIndex ?? $data.length - 1;
    if (event.key === "ArrowLeft") activeIndex = Math.max(0, current - 1);
    else if (event.key === "ArrowRight") activeIndex = Math.min($data.length - 1, current + 1);
    else if (event.key === "Home") activeIndex = 0;
    else if (event.key === "End") activeIndex = $data.length - 1;
    else if (event.key === "Escape") { activeIndex = null; event.stopPropagation(); }
    else return;
    event.preventDefault();
  }
</script>

{#each grid as fraction}
  {@const count = maximum * fraction}
  <line class="grid horizontal" x1="0" x2={$width} y1={$yScale(count)} y2={$yScale(count)} />
  <text x={$width + 10} y={$yScale(count) + 4}>{count.toLocaleString()}</text>
  <line class="grid vertical" x1={$width * fraction} x2={$width * fraction} y1="0" y2={$height} />
{/each}
<path class="activityLine" d={line} />
{#each $data.filter((point) => point.count > 0) as point}
  <circle class="activityPoint" cx={$xGet(point)} cy={$yGet(point)} r="3">
    <title>{timeLabel(point.time)} · {countLabel(point.count)}</title>
  </circle>
{/each}
{#if activePoint}
  <line class="hoverGuide" x1={$xGet(activePoint)} x2={$xGet(activePoint)} y1="0" y2={$height} aria-hidden="true" />
  <circle class="hoverPoint" cx={$xGet(activePoint)} cy={$yGet(activePoint)} r="4" aria-hidden="true" />
{/if}
<rect class="interaction" width={$width} height={$height} role="slider" tabindex="0" aria-label={label} aria-orientation="horizontal" aria-valuemin="0" aria-valuemax={Math.max(0, $data.length - 1)} aria-valuenow={activeIndex ?? 0} aria-valuetext={activePoint ? `${timeLabel(activePoint.time)} · ${countLabel(activePoint.count)}` : label} on:pointermove={inspectPointer} on:pointerdown={inspectPointer} on:pointerleave={(event) => { if (event.pointerType !== "touch") activeIndex = null; }} on:focus={() => { activeIndex ??= $data.length - 1; }} on:blur={() => { activeIndex = null; }} on:keydown={inspectKeyboard} />

<style>
  .grid { stroke: var(--color-border); stroke-width: .7; }
  text { fill: var(--color-text-faint); font-family: var(--font-mono); font-size: 11px; }
  .activityLine { fill: none; stroke: var(--color-accent); stroke-width: 2; stroke-linecap: round; stroke-linejoin: round; }
  .activityPoint, .hoverPoint { fill: var(--color-accent); }
  .hoverPoint { stroke: var(--color-paper); stroke-width: 2; pointer-events: none; }
  .hoverGuide { stroke: var(--color-text-faint); stroke-width: 1; stroke-dasharray: 3 3; pointer-events: none; }
  .interaction { fill: transparent; cursor: crosshair; outline: none; }
  .interaction:focus-visible { stroke: var(--color-focus); stroke-width: 1; }
</style>
