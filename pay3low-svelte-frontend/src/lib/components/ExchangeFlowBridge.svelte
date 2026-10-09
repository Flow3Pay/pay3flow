<script lang="ts">
  import { swapIcon } from "$lib/icons";
  export let label: string;
  export let reversed = false;
  export let onSwap: () => void;
</script>

<div class="flowBridge">
  <span class="bridgeLine" aria-hidden="true"></span>
  <button type="button" class="bridgeIcon" class:bridgeIconReversed={reversed} on:click={onSwap} aria-label={label} title={label}><img src={swapIcon} alt="" width="18" height="18" aria-hidden="true" /></button>
  <slot />
</div>

<style>
  .flowBridge { position: relative; z-index: 4; display: flex; height: 31px; align-items: center; justify-content: center; }
  .bridgeLine { position: absolute; top: 50%; right: 0; left: 0; height: 1px; background: var(--exchange-line); pointer-events: none; }
  .bridgeIcon { position: relative; z-index: 1; display: grid; width: 27px; height: 27px; place-items: center; border: 1px solid var(--exchange-arrow-border); border-radius: 50%; background: var(--exchange-arrow-bg); color: var(--color-accent-text); cursor: pointer; box-shadow: none; transition: transform .2s ease; }
  .bridgeIcon:hover { transform: translateY(-1px) scale(1.04); }
  .bridgeIcon:active { transform: scale(.96); }
  .bridgeIcon img { width: 14px; height: 14px; filter: brightness(0) saturate(100%) invert(51%) sepia(23%) saturate(1100%) hue-rotate(37deg) brightness(88%) contrast(88%); transition: transform .32s cubic-bezier(.22, 1, .36, 1); }
  .bridgeIconReversed img { transform: rotate(180deg); }
  :global(html[data-theme="dark"]) .bridgeIcon img { filter: brightness(0) saturate(100%) invert(78%) sepia(39%) saturate(849%) hue-rotate(35deg) brightness(106%) contrast(102%); }
  @media (max-width: 640px) { .flowBridge { height: 44px; margin: 10px 0 4px; } }
  @media (prefers-reduced-motion: reduce) { .bridgeIcon, .bridgeIcon img { transition: none; } }
</style>
