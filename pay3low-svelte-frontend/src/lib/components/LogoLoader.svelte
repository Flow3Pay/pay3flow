<script lang="ts">
  /**
   * Pay3Flow mark loader.
   *
   * The real mark is revealed by three thick mask strokes that follow the
   * ribbon of the logo, so it looks like the mark is being drawn:
   *
   *   1. top bar → right curve → back down-left along the upper bowl;
   *   2. the diagonal stem down to the lower-left tip;
   *   3. the "3": middle bar → right bowl → bottom bar to the left.
   */
  let {
    size = 44,
    label = "Loading…",
    duration = 1800,
    ghost = true,
  }: {
    /** Rendered width and height in pixels. */
    size?: number;
    /** Accessible label for the loader. */
    label?: string;
    /** One full reveal cycle in milliseconds. */
    duration?: number;
    /** Show the full mark faintly behind the animated reveal. */
    ghost?: boolean;
  } = $props();

  // Unique per instance and stable across SSR hydration.
  const maskId = $props.id();
</script>

<svg
  role="img"
  aria-label={label}
  width={size}
  height={size}
  viewBox="0 0 500 500"
  fill="none"
  xmlns="http://www.w3.org/2000/svg"
  class="logoLoader"
  style:--duration="{duration}ms"
>
  <defs>
    <mask id={maskId} maskUnits="userSpaceOnUse" x="0" y="0" width="500" height="500">
      <rect width="500" height="500" fill="#000" />
      <path
        class="trace t1"
        pathLength="1"
        d="M148 135H300C330 135 350 148 358 170C368 196 352 214 328 225L271 251"
      />
      <path class="trace t2" pathLength="1" d="M237 200L118 280" />      <path class="trace t3" pathLength="1" d="M200 288L330 276C350 278 360 300 355 320C350 340 332 350 305 350L150 368" />
      <rect class="fillAll" width="500" height="500" fill="#fff" />
    </mask>
  </defs>

  {#if ghost}
    <image class="ghost" href="/icons/assets/pay3flow-mark.svg" width="500" height="500" />
  {/if}

  <g class="mark">
    <image
      href="/icons/assets/pay3flow-mark.svg"
      width="500"
      height="500"
      mask="url(#{maskId})"
    />
  </g>
</svg>

<style>
  .logoLoader {
    display: block;
    overflow: visible;
  }

  .ghost {
    opacity: 0.16;
  }

  .mark {
    animation: logoCycle var(--duration, 1800ms) linear infinite;
  }

  .trace {
    fill: none;
    stroke: #fff;
    stroke-width: 100;
    stroke-linecap: round;
    stroke-linejoin: round;
    stroke-dasharray: 1;
    stroke-dashoffset: 1;
    animation-duration: var(--duration, 1800ms);
    animation-timing-function: linear;
    animation-iteration-count: infinite;
  }

  .t1 {
    animation-name: traceUpper;
  }

  .t2 {
    animation-name: traceStem;
  }

  .t3 {
    animation-name: traceThree;
  }

  .fillAll {
    opacity: 0;
    animation: fillAll var(--duration, 1800ms) linear infinite;
  }

  @keyframes traceUpper {
    0%,
    6% {
      stroke-dashoffset: 1;
    }
    38%,
    100% {
      stroke-dashoffset: 0;
    }
  }

  @keyframes traceStem {
    0%,
    38% {
      stroke-dashoffset: 1;
    }
    52%,
    100% {
      stroke-dashoffset: 0;
    }
  }

  @keyframes traceThree {
    0%,
    52% {
      stroke-dashoffset: 1;
    }
    82%,
    100% {
      stroke-dashoffset: 0;
    }
  }

  @keyframes fillAll {
    0%,
    82% {
      opacity: 0;
    }
    92%,
    100% {
      opacity: 1;
    }
  }

  @keyframes logoCycle {
    0%,
    92% {
      opacity: 1;
    }
    97%,
    100% {
      opacity: 0;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .mark,
    .trace,
    .fillAll {
      animation: none;
    }

    .trace {
      stroke-dashoffset: 0;
    }

    .fillAll {
      opacity: 1;
    }

    .mark {
      opacity: 1;
    }
  }
</style>
