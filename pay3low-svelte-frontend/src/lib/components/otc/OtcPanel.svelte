<script lang="ts">
  import { locale } from "$lib/i18n";
  import { otcCopy } from "$lib/otc/copy";
  export let id: string;
  export let title: string;
  export let subtitle = "";
  export let expanded = true;
  $: copy = otcCopy($locale);
</script>
<section class="otcPanel" class:collapsed={!expanded} aria-label={title}>
  <div class="panelHead">
    <div class="panelTitle"><h2>{title}</h2>{#if subtitle}<span>{subtitle}</span>{/if}</div>
    <div class="panelActions"><slot name="actions" /><button class="collapseButton" type="button" aria-label={`${expanded ? copy.collapse : copy.expand} ${title}`} aria-controls={id} aria-expanded={expanded} on:click={() => expanded = !expanded}><svg class:closed={!expanded} width="16" height="16" viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="m6 9 6 6 6-6" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" /></svg></button></div>
  </div>
  <div id={id} class="panelBody" hidden={!expanded}><slot /></div>
</section>
<style>
  .otcPanel { min-width: 0; border: 1px solid var(--color-border); border-radius: var(--radius-card); background: var(--color-paper); overflow: hidden; }
  .panelHead { display: flex; align-items: center; justify-content: space-between; gap: 10px; min-height: 57px; padding: 13px 17px; border-bottom: 1px solid var(--color-border); }
  .collapsed .panelHead { border-bottom-color: transparent; }
  .panelTitle, .panelActions { display: flex; align-items: center; gap: 10px; min-width: 0; }
  h2 { font-size: 13px; font-weight: 800; white-space: nowrap; }
  .panelTitle > span { font-family: var(--font-mono); font-size: 10px; color: var(--color-text-soft); }
  .collapseButton { display: grid; width: 29px; height: 29px; place-items: center; border-radius: 7px; color: var(--color-text-soft); }
  .collapseButton:hover { background: var(--color-panel); color: var(--color-text); }
  svg { transition: transform .2s; transform: rotate(180deg); }
  svg.closed { transform: rotate(0); }
  .panelBody { min-width: 0; }
  @media (pointer: coarse) { .collapseButton { width: 36px; height: 36px; } }
  @media (prefers-reduced-motion: reduce) { svg { transition: none; } }
</style>
