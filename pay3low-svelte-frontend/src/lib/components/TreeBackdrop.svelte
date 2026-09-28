<script lang="ts">
  import { onMount } from "svelte";

  let canvas: HTMLCanvasElement;

  type Leaf = {
    x: number;
    y: number;
    vx: number;
    vy: number;
    size: number;
    rotation: number;
    spin: number;
    phase: number;
    weight: number;
    color: string;
  };

  const leafColors = ["#b5f500", "#a3df25", "#8fbd28", "#c4f35a"];
  const emitters = [
    { x: 0.12, y: 0.23 },
    { x: 0.25, y: 0.29 },
    { x: 0.36, y: 0.34 },
    { x: 0.48, y: 0.39 },
    { x: 0.21, y: 0.47 },
    { x: 0.41, y: 0.5 },
    { x: 0.56, y: 0.56 },
    { x: 0.3, y: 0.6 },
  ];

  let leaves: Leaf[] = [];
  let width = 0;
  let height = 0;
  let dpr = 1;
  let animationFrame = 0;
  let lastTime = 0;

  function randomBetween(min: number, max: number) {
    return min + Math.random() * (max - min);
  }

  function resetLeaf(leaf: Leaf, initial = false) {
    const emitter = emitters[Math.floor(Math.random() * emitters.length)];
    leaf.x = (emitter.x + randomBetween(-0.035, 0.035)) * width;
    leaf.y = (emitter.y + randomBetween(-0.025, 0.025)) * height;
    if (initial) leaf.y = randomBetween(0.14, 0.78) * height;
    leaf.vx = randomBetween(-0.18, 0.18);
    leaf.vy = randomBetween(0.18, 0.48);
    leaf.size = randomBetween(10, 22);
    leaf.rotation = randomBetween(-Math.PI, Math.PI);
    leaf.spin = randomBetween(-0.035, 0.035);
    leaf.phase = randomBetween(0, Math.PI * 2);
    leaf.weight = randomBetween(0.7, 1.35);
    leaf.color = leafColors[Math.floor(Math.random() * leafColors.length)];
  }

  function resize() {
    if (!canvas) return;
    const bounds = canvas.getBoundingClientRect();
    width = bounds.width;
    height = bounds.height;
    dpr = Math.min(window.devicePixelRatio || 1, 2);
    canvas.width = Math.round(width * dpr);
    canvas.height = Math.round(height * dpr);

    if (!leaves.length) {
      const count = width < 700 ? 34 : 72;
      leaves = Array.from({ length: count }, () => {
        const leaf = {} as Leaf;
        resetLeaf(leaf, true);
        return leaf;
      });
    }
  }

  function drawLeaf(context: CanvasRenderingContext2D, leaf: Leaf) {
    context.save();
    context.translate(leaf.x, leaf.y);
    context.rotate(leaf.rotation);
    context.scale(leaf.size / 8, leaf.size / 8);
    context.beginPath();
    context.moveTo(0, -1);
    context.bezierCurveTo(4.6, -7, 8.2, -5.4, 7.2, -1.2);
    context.bezierCurveTo(6.4, 2.8, 2.3, 5.8, 0, 7);
    context.bezierCurveTo(-0.6, 3.2, -0.5, 1.1, 0, -1);
    context.fillStyle = leaf.color;
    context.globalAlpha = 1;
    context.shadowColor = "rgba(38, 55, 20, 0.2)";
    context.shadowBlur = 3;
    context.fill();
    context.restore();
  }

  function animate(time: number) {
    if (!canvas) return;
    const context = canvas.getContext("2d");
    if (!context) return;
    const delta = Math.min((time - lastTime) / 16.67 || 1, 2);
    lastTime = time;

    context.setTransform(dpr, 0, 0, dpr, 0, 0);
    context.clearRect(0, 0, width, height);

    for (const leaf of leaves) {
      const breeze = Math.sin(time * 0.00042 + leaf.phase) * 0.22;
      const gust = Math.sin(time * 0.00115 + leaf.phase * 1.7) * 0.11;
      const vortex = Math.sin(leaf.y * 0.012 + time * 0.0007 + leaf.phase) * 0.08;
      const wind = 0.18 + breeze + gust + vortex;

      leaf.vx += (wind - leaf.vx) * 0.018 * delta;
      leaf.vy += 0.009 * leaf.weight * delta;
      leaf.vy *= 0.998;
      leaf.x += leaf.vx * delta;
      leaf.y += leaf.vy * delta;
      leaf.rotation += leaf.spin * delta + leaf.vx * 0.018;

      if (leaf.y > height + 30 || leaf.x < -50 || leaf.x > width + 50) resetLeaf(leaf);
      drawLeaf(context, leaf);
    }

    animationFrame = requestAnimationFrame(animate);
  }

  onMount(() => {
    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
    resize();
    if (!reducedMotion.matches) animationFrame = requestAnimationFrame(animate);

    const resizeObserver = new ResizeObserver(resize);
    resizeObserver.observe(canvas);
    return () => {
      cancelAnimationFrame(animationFrame);
      resizeObserver.disconnect();
    };
  });
</script>

<div class="treeBackdrop" aria-hidden="true">
  <svg class="treeArtwork" viewBox="0 0 760 900" preserveAspectRatio="xMidYMid meet">
    <defs>
      <!-- The logo's dark background is removed by green-channel alpha; the tree itself stays opaque. -->
      <filter id="logo-tree-cutout" color-interpolation-filters="sRGB">
        <feColorMatrix type="matrix" values="1 0 0 0 0  0 1 0 0 0  0 0 1 0 0  -8 8 0 0 0" />
        <feComponentTransfer>
          <feFuncA type="table" tableValues="0 0 0 1 1" />
        </feComponentTransfer>
      </filter>
      <path id="extra-leaf" d="M0 0C8-13 21-16 27-11C24 2 14 11 0 14C2 8 2 4 0 0Z" />
      <g id="flower-center" fill="#262D1F" stroke="#262D1F" stroke-width="1.35" stroke-linecap="round">
        <circle r="2.15" stroke="none" />
        <path d="M0-1V-5M0 1V5M-1 0H-5M1 0H5M-.8-.8-3.8-3.8M.8.8 3.8 3.8M.8-.8 3.8-3.8M-.8.8-3.8 3.8" />
      </g>
    </defs>

    <image
      href="/icons/assets/pay3flow_logo.svg"
      x="0"
      y="90"
      width="760"
      height="760"
      preserveAspectRatio="xMidYMid meet"
      filter="url(#logo-tree-cutout)"
    />

    <g class="additionalBranches" fill="none" stroke="#a4f92d" stroke-linecap="round" stroke-linejoin="round">
      <path d="M135 735C157 655 212 607 287 588C354 571 403 530 439 475" stroke-width="19" />
      <path d="M196 555C253 515 315 483 358 427C391 384 425 359 480 342" stroke-width="15" />
      <path d="M205 435C257 382 299 322 350 280C382 254 414 239 458 225" stroke-width="12" />
      <path d="M395 526C459 495 511 452 550 398C577 360 609 345 660 334" stroke-width="15" />
      <path d="M488 414C535 381 573 342 604 291" stroke-width="10" />
      <path d="M280 585C328 606 369 628 402 667" stroke-width="9" />
      <path d="M344 431C312 402 292 369 283 332M438 475C472 445 494 411 503 374M550 398C594 391 625 372 651 346M358 427C377 399 386 371 386 340" stroke="#79a82b" stroke-width="5" />
    </g>

    <g class="additionalBranchLeaves" fill="#b5f500">
      <use href="#extra-leaf" transform="translate(458 225) rotate(-48) scale(1.15)" />
      <use href="#extra-leaf" transform="translate(480 342) rotate(-24) scale(0.95)" />
      <use href="#extra-leaf" transform="translate(660 334) rotate(-42) scale(1.05)" />
      <use href="#extra-leaf" transform="translate(604 291) rotate(-58) scale(0.8)" />
      <use href="#extra-leaf" transform="translate(402 667) rotate(-52) scale(0.82)" />
      <use href="#extra-leaf" transform="translate(283 332) rotate(-66) scale(0.72)" />
      <use href="#extra-leaf" transform="translate(386 340) rotate(-34) scale(0.68)" />
    </g>

    <g class="additionalLeaves" fill="#b5f500">
      <use href="#extra-leaf" transform="translate(95 380) rotate(-38) scale(1.1)" />
      <use href="#extra-leaf" transform="translate(165 292) rotate(-62) scale(0.78)" />
      <use href="#extra-leaf" transform="translate(381 256) rotate(-28) scale(0.86)" />
      <use href="#extra-leaf" transform="translate(584 304) rotate(-44) scale(0.92)" />
      <use href="#extra-leaf" transform="translate(690 420) rotate(-60) scale(0.72)" />
      <use href="#extra-leaf" transform="translate(310 604) rotate(-48) scale(0.82)" />
    </g>

    <g class="flowerCenters">
      <use href="#flower-center" transform="translate(266 300) scale(1.05)" />
      <use href="#flower-center" transform="translate(559 328) scale(0.52)" />
      <use href="#flower-center" transform="translate(453 426) scale(1.15)" />
      <use href="#flower-center" transform="translate(459 573) scale(0.5)" />
    </g>
  </svg>
  <canvas bind:this={canvas}></canvas>
</div>

<style>
  .treeBackdrop {
    position: fixed;
    inset: 0;
    z-index: 0;
    overflow: hidden;
    pointer-events: none;
  }

  .treeArtwork {
    position: absolute;
    top: 34px;
    left: -130px;
    width: min(1250px, 94vw);
    height: calc(100vh - 42px);
    min-height: 0;
    overflow: visible;
  }

  .additionalLeaves,
  .additionalBranchLeaves {
    filter: drop-shadow(1px 2px 2px rgba(38, 55, 20, 0.16));
  }

  canvas {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }

  @media (max-width: 700px) {
    .treeArtwork {
      top: 132px;
      left: -230px;
      width: 760px;
      height: 740px;
      min-height: 0;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    canvas { display: none; }
  }
</style>
