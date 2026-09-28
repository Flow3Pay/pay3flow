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
    { x: 0.64, y: 0.23 },
    { x: 0.74, y: 0.29 },
    { x: 0.58, y: 0.36 },
    { x: 0.82, y: 0.39 },
    { x: 0.69, y: 0.48 },
    { x: 0.9, y: 0.52 },
    { x: 0.52, y: 0.55 },
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
    if (initial) leaf.y = randomBetween(0.12, 0.78) * height;
    leaf.vx = randomBetween(-0.18, 0.18);
    leaf.vy = randomBetween(0.18, 0.48);
    leaf.size = randomBetween(4.5, 10.5);
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
      const count = width < 700 ? 26 : 58;
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
    context.globalAlpha = 0.78;
    context.shadowColor = "rgba(90, 120, 20, 0.18)";
    context.shadowBlur = 5;
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
  <div class="treeAtmosphere"></div>
  <svg class="treeArtwork" viewBox="0 0 760 920" preserveAspectRatio="xMidYMid meet">
    <defs>
      <filter id="tree-shadow" x="-30%" y="-30%" width="160%" height="160%">
        <feDropShadow dx="10" dy="18" stdDeviation="16" flood-color="#192317" flood-opacity="0.16" />
      </filter>
      <linearGradient id="trunk-gradient" x1="0" y1="0" x2="1" y2="1">
        <stop offset="0" stop-color="#253221" />
        <stop offset="0.52" stop-color="#1b241a" />
        <stop offset="1" stop-color="#3c5224" />
      </linearGradient>
      <path id="leaf-shape" d="M0 0C8-13 21-16 27-11C24 2 14 11 0 14C2 8 2 4 0 0Z" />
    </defs>

    <g class="treeStructure" filter="url(#tree-shadow)">
      <path class="trunk" d="M448 933C404 844 390 755 416 664C433 605 461 546 464 481C468 403 449 319 472 236" />
      <path class="branch" d="M451 594C393 539 318 493 218 467C161 452 107 418 75 365" />
      <path class="branch" d="M459 530C530 480 585 414 631 333C658 286 694 255 747 233" />
      <path class="branch" d="M464 458C398 416 351 364 320 294C298 245 266 208 219 176" />
      <path class="branch" d="M433 679C363 637 297 615 220 622C167 627 126 610 92 581" />
      <path class="twig" d="M215 466C180 434 150 407 122 367M161 449C136 431 111 419 80 413M318 494C287 463 269 431 253 391M584 414C610 383 629 346 641 302M632 333C663 323 690 301 713 273M348 378C330 344 317 314 313 278M298 616C274 585 258 551 251 515M220 622C187 604 160 583 138 548" />
      <path class="twig" d="M461 371C499 337 528 309 544 271M471 240C497 213 518 183 526 147M388 442C355 427 328 406 307 379M413 664C381 651 350 643 315 644" />

      <g class="treeLeaves" fill="var(--tree-leaf)">
        <use href="#leaf-shape" transform="translate(70 349) rotate(-37) scale(1.35)" />
        <use href="#leaf-shape" transform="translate(119 366) rotate(-23) scale(0.92)" />
        <use href="#leaf-shape" transform="translate(145 422) rotate(-54) scale(0.82)" />
        <use href="#leaf-shape" transform="translate(186 454) rotate(-28) scale(1.05)" />
        <use href="#leaf-shape" transform="translate(215 174) rotate(-58) scale(1.1)" />
        <use href="#leaf-shape" transform="translate(260 208) rotate(-35) scale(0.84)" />
        <use href="#leaf-shape" transform="translate(306 286) rotate(-68) scale(1.18)" />
        <use href="#leaf-shape" transform="translate(319 377) rotate(-23) scale(0.86)" />
        <use href="#leaf-shape" transform="translate(505 150) rotate(-57) scale(1.15)" />
        <use href="#leaf-shape" transform="translate(541 272) rotate(-25) scale(0.88)" />
        <use href="#leaf-shape" transform="translate(641 301) rotate(-39) scale(1.26)" />
        <use href="#leaf-shape" transform="translate(708 274) rotate(-58) scale(0.86)" />
        <use href="#leaf-shape" transform="translate(713 232) rotate(-20) scale(1.06)" />
        <use href="#leaf-shape" transform="translate(248 515) rotate(-39) scale(0.9)" />
        <use href="#leaf-shape" transform="translate(314 642) rotate(-53) scale(1.08)" />
        <use href="#leaf-shape" transform="translate(138 548) rotate(-28) scale(0.98)" />
      </g>
    </g>

    <g class="treeHighlights" fill="none" stroke="var(--tree-highlight)" stroke-linecap="round">
      <path d="M438 886C408 786 422 697 450 625" stroke-width="8" stroke-opacity="0.32" />
      <path d="M445 552C386 506 313 472 230 451" stroke-width="5" stroke-opacity="0.24" />
      <path d="M481 497C548 449 590 393 623 334" stroke-width="4" stroke-opacity="0.23" />
    </g>
  </svg>
  <canvas bind:this={canvas}></canvas>
</div>

<style>
  .treeBackdrop {
    --tree-leaf: rgba(181, 245, 0, 0.82);
    --tree-highlight: #b5f500;
    position: fixed;
    inset: 0;
    z-index: 0;
    overflow: hidden;
    pointer-events: none;
    opacity: 0.68;
  }

  .treeAtmosphere {
    position: absolute;
    top: 4%;
    right: 0;
    width: min(920px, 82vw);
    height: 86%;
    border-radius: 50%;
    background: radial-gradient(ellipse at 62% 52%, rgba(181, 245, 0, 0.065), transparent 68%);
    filter: blur(10px);
  }

  .treeArtwork {
    position: absolute;
    top: 76px;
    right: -100px;
    width: min(850px, 77vw);
    height: calc(100vh - 90px);
    min-height: 680px;
    color: rgba(27, 38, 26, 0.22);
  }

  .treeStructure .trunk {
    fill: none;
    stroke: url(#trunk-gradient);
    stroke-linecap: round;
    stroke-linejoin: round;
    stroke-width: 74;
  }

  .treeStructure .branch {
    fill: none;
    stroke: url(#trunk-gradient);
    stroke-linecap: round;
    stroke-linejoin: round;
    stroke-width: 28;
  }

  .treeStructure .twig {
    fill: none;
    stroke: #344723;
    stroke-linecap: round;
    stroke-linejoin: round;
    stroke-width: 10;
  }

  .treeLeaves {
    opacity: 0.74;
  }

  .treeLeaves use:nth-child(3n) { opacity: 0.72; }
  .treeLeaves use:nth-child(4n) { opacity: 0.48; }

  .treeHighlights { display: block; }

  canvas {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }

  :global(html[data-theme="dark"]) .treeBackdrop {
    --tree-leaf: rgba(181, 245, 0, 0.7);
    opacity: 0.8;
  }

  :global(html[data-theme="dark"]) .treeAtmosphere {
    background: radial-gradient(ellipse at 62% 52%, rgba(181, 245, 0, 0.09), transparent 68%);
  }

  :global(html[data-theme="dark"]) .treeArtwork { color: rgba(181, 245, 0, 0.14); }

  @media (max-width: 700px) {
    .treeBackdrop { opacity: 0.42; }
    .treeArtwork {
      top: 170px;
      right: -290px;
      width: 690px;
      height: 740px;
      min-height: 0;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    canvas { display: none; }
  }
</style>
