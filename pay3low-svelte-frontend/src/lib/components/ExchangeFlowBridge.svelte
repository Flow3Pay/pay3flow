<script lang="ts">
  export let label: string;
  export let reversed = false;
  export let onSwap: () => void;
</script>

<div class="flowBridge">
  <button type="button" class="bridgeIcon" class:bridgeIconReversed={reversed} on:click={onSwap} aria-label={label} title={label}><svg width="18" height="18" viewBox="0 0 24 24" aria-hidden="true" focusable="false"><path d="M12 2C6.48 2 2 6.48 2 12C2 17.52 6.48 22 12 22C17.52 22 22 17.52 22 12C22 6.48 17.52 2 12 2ZM17.85 14.1C17.81 14.19 17.76 14.27 17.69 14.34L14.65 17.38C14.5 17.53 14.31 17.6 14.12 17.6C13.93 17.6 13.74 17.53 13.59 17.38C13.3 17.09 13.3 16.61 13.59 16.32L15.35 14.56H6.85C6.44 14.56 6.1 14.22 6.1 13.81C6.1 13.4 6.44 13.06 6.85 13.06H17.16C17.26 13.06 17.35 13.08 17.45 13.12C17.63 13.2 17.78 13.34 17.86 13.53C17.92 13.71 17.92 13.92 17.85 14.1ZM17.15 10.93H6.85C6.75 10.93 6.65 10.91 6.56 10.87C6.38 10.79 6.23 10.65 6.15 10.46C6.07 10.28 6.07 10.07 6.15 9.89C6.19 9.8 6.24 9.72 6.31 9.65L9.35 6.61C9.64 6.32 10.12 6.32 10.41 6.61C10.7 6.9 10.7 7.38 10.41 7.67L8.66 9.43H17.16C17.57 9.43 17.91 9.77 17.91 10.18C17.91 10.59 17.57 10.93 17.15 10.93Z"/></svg></button>
  <slot />
</div>

<style>
  .flowBridge { position: relative; z-index: 4; display: flex; height: 31px; align-items: center; justify-content: center; }
  .bridgeIcon { position: relative; z-index: 1; display: grid; width: 27px; height: 27px; place-items: center; border: 1px solid var(--exchange-arrow-border); border-radius: 50%; background: var(--exchange-arrow-bg); color: var(--color-accent-text); cursor: pointer; box-shadow: none; transition: transform .2s ease; }
  .bridgeIcon:hover { transform: translateY(-1px) scale(1.04); }
  .bridgeIcon:active { transform: scale(.96); }
  .bridgeIcon svg { width: 14px; height: 14px; fill: currentColor; transition: transform .32s cubic-bezier(.22, 1, .36, 1); }
  .bridgeIconReversed svg { transform: rotate(180deg); }
  @media (max-width: 640px) { .flowBridge { height: 44px; margin: 10px 0 4px; } }
  @media (prefers-reduced-motion: reduce) { .bridgeIcon, .bridgeIcon svg { transition: none; } }
</style>
