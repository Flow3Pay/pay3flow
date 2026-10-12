<script lang="ts">
  import { useStore } from "@xyflow/svelte";
  import { emptyWallSlots, type WallCell } from "$lib/trends/background";
  let { cells, brickHeight }: { cells: WallCell[]; brickHeight: number } = $props();
  const store = useStore();
  const patternId = `trend-bricks-${store.flowId}`;
  const maskId = `trend-occupied-${store.flowId}`;
  let zoom = $derived(store.viewport.zoom);
  let widths = $derived([...new Set(cells.map((cell) => cell.width))]);
  let patternCells = $derived.by(() => {
    let left = 0;
    return widths.map((width) => { const cell = { left, width }; left += width + 6; return cell; });
  });
  let cycle = $derived(Math.max(1, widths.reduce((sum, width) => sum + width + 6, 0)));
  let pitch = $derived(brickHeight + 8);
  let occupiedRows = $derived([...new Set(cells.map((cell) => cell.row))]);
  let slots = $derived(emptyWallSlots(cells, widths, -store.viewport.x / zoom, (store.width - store.viewport.x) / zoom, -store.viewport.y / zoom, (store.height - store.viewport.y) / zoom, brickHeight));
</script>

<svg class="brickBackground" data-testid="trend-brick-background" aria-hidden="true">
  <defs>
    <pattern id={patternId} x={store.viewport.x % (cycle * zoom)} y={store.viewport.y % (2 * pitch * zoom)} width={cycle * zoom} height={2 * pitch * zoom} patternUnits="userSpaceOnUse">
      <g transform={`scale(${zoom})`}>
        {#each patternCells as cell}
          <rect x={cell.left} y="0" width={cell.width} height={brickHeight} rx="5" />
          <rect x={cell.left + 46} y={pitch} width={cell.width} height={brickHeight} rx="5" />
          <rect x={cell.left + 46 - cycle} y={pitch} width={cell.width} height={brickHeight} rx="5" />
        {/each}
      </g>
    </pattern>
    <mask id={maskId} maskUnits="userSpaceOnUse" x="0" y="0" width="100%" height="100%">
      <rect width="100%" height="100%" fill="white" />
      {#each occupiedRows as row}<rect x="0" y={store.viewport.y + row * pitch * zoom - zoom} width="100%" height={(pitch + 1) * zoom} fill="black" />{/each}
    </mask>
  </defs>
  <rect width="100%" height="100%" fill={`url(#${patternId})`} mask={`url(#${maskId})`} />
  <g class="emptySlots" transform={`translate(${store.viewport.x}, ${store.viewport.y}) scale(${zoom})`}>
    {#each slots as slot}<rect class="emptySlot" x={slot.left} y={slot.top} width={slot.width} height={slot.height} rx="5" />{/each}
  </g>
</svg>

<style>
  .brickBackground { position: absolute; inset: 0; width: 100%; height: 100%; pointer-events: none; }
  pattern rect, .emptySlot { fill: none; stroke: var(--color-border); stroke-width: 1; }
</style>
