<script lang="ts">
  /**
   * Pay3Flow mark loader.
   *
   * The mark draws itself over 1.8s, then the clip loops. Both files are
   * transparent, so the loader drops onto any panel without a backdrop of its
   * own: the animated one is a looping APNG, the still one keeps the finished
   * mark for reduced motion.
   */
  let {
    size = 44,
    label = "Loading…",
  }: {
    /** Rendered width and height in pixels. */
    size?: number;
    /** Accessible label for the loader. */
    label?: string;
  } = $props();
</script>

<picture class="logoLoader">
  <source
    media="(prefers-reduced-motion: reduce)"
    srcset="/animation/pay3flow-loader-still.png"
  />
  <img
    src="/animation/pay3flow-loader.png"
    alt={label}
    width={size}
    height={size}
    draggable="false"
  />
</picture>

<style>
  .logoLoader {
    display: block;
  }

  .logoLoader img {
    display: block;
    user-select: none;
    -webkit-user-drag: none;
  }
</style>
