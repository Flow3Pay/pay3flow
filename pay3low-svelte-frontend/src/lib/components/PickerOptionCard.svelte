<script lang="ts">
  export let name: string;
  export let meta: string | undefined = undefined;
  export let iconUrl: string | null = null;
  export let initials: string;
  export let color = "#171a17";
  export let selected = false;
  export let category: string | undefined = undefined;
  export let onSelect: () => void;

  let imageFailed = false;
  $: if (iconUrl) imageFailed = false;
</script>

<button type="button" role="option" aria-selected={selected} class="optionCard" data-selected={selected || undefined} on:click={onSelect}>
  <span class="optionLogo" style:background-color={iconUrl && !imageFailed ? "transparent" : color} aria-hidden="true">
    {#if iconUrl && !imageFailed}
      <img src={iconUrl} alt="" width="42" height="42" loading="lazy" decoding="async" on:error={() => imageFailed = true} />
    {:else}
      <span>{initials}</span>
    {/if}
  </span>
  <span class="optionCopy">
    <span class="optionName">{name}</span>
    {#if meta}<span class="optionMeta">{meta}</span>{/if}
  </span>
  {#if category}<span class="category">{category}</span>{/if}
  {#if selected}
    <svg class="check" width="20" height="20" viewBox="0 0 20 20" fill="none" aria-hidden="true"><circle cx="10" cy="10" r="10" fill="currentColor" /><path d="m6 10.2 2.7 2.5 5.3-5.6" stroke="white" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" /></svg>
  {/if}
</button>

<style>
  .optionCard { display: flex; width: 100%; min-height: 48px; align-items: center; gap: 10px; padding: 7px 8px; border: 1px solid transparent; border-radius: 7px; text-align: left; contain: layout paint; transition: background .13s ease; }
  .optionCard:hover, .optionCard:focus-visible { background: var(--color-panel-soft); }.optionCard[data-selected] { background: var(--color-panel-soft); }.optionCard:focus-visible { outline: 2px solid var(--color-focus); outline-offset: -2px; }
  .optionLogo { position: relative; display: grid; width: 32px; height: 32px; flex: 0 0 auto; place-items: center; border: 1px solid var(--color-border); border-radius: 8px; color: #fff; font-size: 10px; font-weight: 650; }.optionLogo img { width: 24px; height: 24px; border-radius: 5px; object-fit: contain; }
  .optionCopy { display: flex; min-width: 0; flex: 1; flex-direction: column; gap: 2px; }.optionName { overflow: hidden; font-size: 13px; font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }.optionMeta { color: var(--color-text-faint); font-size: 11px; line-height: 1.4; overflow-wrap: anywhere; }
  .category { max-width: 90px; color: var(--color-text-faint); text-align: right; font-size: 10px; line-height: 1.4; }.check { width: 16px; height: 16px; flex: 0 0 auto; color: var(--color-accent-text); }
  @media (max-width: 380px) { .category { max-width: 65px; font-size: 9px; } }
  @media (prefers-reduced-motion: reduce) { .optionCard { transition: none; } }
</style>
