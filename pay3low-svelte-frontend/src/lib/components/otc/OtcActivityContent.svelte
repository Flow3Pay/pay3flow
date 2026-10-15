<script lang="ts">
  import { onMount, tick } from "svelte";
  export let activeKey: string;
  export let duration = 240;
  let viewport: HTMLDivElement, content: HTMLDivElement;
  let previousKey = activeKey;
  let animation: Animation | undefined;
  function fit() { if (viewport && content) viewport.style.height = `${content.getBoundingClientRect().height}px`; }
  async function reveal() {
    await tick();
    if (!content?.isConnected) return;
    fit(); animation?.cancel();
    if (duration) animation = content.animate([{ opacity: .35, transform: "translateY(6px)" }, { opacity: 1, transform: "translateY(0)" }], { duration, easing: "cubic-bezier(.22, 1, .36, 1)" });
  }
  $: if (activeKey !== previousKey) { previousKey = activeKey; void reveal(); }
  $: if (!duration) animation?.cancel();
  onMount(() => {
    fit();
    const observer = new ResizeObserver(fit);
    observer.observe(content);
    return () => { observer.disconnect(); animation?.cancel(); };
  });
</script>

<div class="activityViewport" bind:this={viewport} style:--activity-motion={`${duration}ms`}>
  <div bind:this={content}><slot /></div>
</div>

<style>
  .activityViewport { min-width: 0; overflow: clip; transition: height var(--activity-motion) cubic-bezier(.22, 1, .36, 1); }
  @media (prefers-reduced-motion: reduce) { .activityViewport { transition: none; } }
</style>
