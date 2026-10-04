<script lang="ts">
  export let name: string;
  export let meta: string | undefined = undefined;
  export let iconUrl: string | null = null;
  export let initials: string;
  export let color = "#171a17";
  export let selected = false;
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
  {#if selected}
    <svg class="check" width="20" height="20" viewBox="0 0 20 20" fill="none" aria-hidden="true"><circle cx="10" cy="10" r="10" fill="currentColor" /><path d="m6 10.2 2.7 2.5 5.3-5.6" stroke="white" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" /></svg>
  {/if}
</button>

<style>
  .optionCard {
    display: flex;
    width: 100%;
    align-items: center;
    gap: 11px;
    padding: 10px 11px;
    border: 1px solid transparent;
    border-radius: var(--radius-panel);
    text-align: left;
    contain: layout paint;
    transition: background 0.13s ease, border-color 0.13s ease, box-shadow 0.13s ease;
  }

  .optionCard:hover {
    border-color: var(--color-border-strong);
    background: var(--color-panel-soft);
  }

  .optionCard[data-selected] {
    border-color: var(--color-accent-strong);
    background: var(--color-accent-soft);
  }

  .optionLogo {
    position: relative;
    display: grid;
    width: 46px;
    height: 46px;
    flex: 0 0 auto;
    place-items: center;
    border: 2px solid rgba(255, 255, 255, 0.72);
    border-radius: 15px;
    color: #fff;
    box-shadow: 0 6px 15px rgba(20, 23, 19, 0.14);
    font-size: 11px;
    font-weight: 850;
    letter-spacing: 0.03em;
  }

  .optionLogo img {
    position: absolute;
    inset: 5px;
    width: calc(100% - 10px);
    height: calc(100% - 10px);
    border-radius: 10px;
    object-fit: contain;
  }

  .optionCopy {
    display: flex;
    min-width: 0;
    flex: 1;
    flex-direction: column;
    gap: 3px;
  }

  .optionName {
    overflow: hidden;
    font-size: 13px;
    font-weight: 780;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .optionMeta {
    color: var(--color-text-faint);
    font-family: var(--font-mono);
    font-size: 9px;
    font-weight: 600;
  }

  .check {
    flex: 0 0 auto;
    color: var(--color-accent-strong);
  }

  :global(html[data-theme="dark"]) .optionCard:hover {
    border-color: var(--color-border-strong);
    background: var(--color-panel-soft);
  }

  :global(html[data-theme="dark"]) .optionCard[data-selected] {
    border-color: var(--color-accent-strong);
    background: var(--color-accent-soft);
  }

  :global(html[data-theme="dark"]) .optionLogo {
    border-color: rgba(255, 255, 255, 0.12);
  }
</style>
