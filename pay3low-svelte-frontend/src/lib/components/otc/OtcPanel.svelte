<script lang="ts">
  import { locale } from "$lib/i18n";
  import { otcCopy } from "$lib/otc/copy";
  export let id: string;
  export let title: string;
  export let subtitle = "";
  export let expanded = true;
  export let collapsible = true;
  $: copy = otcCopy($locale);
</script>
<section class="otcPanel" class:collapsed={!expanded} aria-label={title}>
  <div class="panelHead">
    <div class="panelTitle"><h2>{title}</h2>{#if subtitle}<span>{subtitle}</span>{/if}</div>
    <div class="panelActions"><slot name="actions" />{#if collapsible}<button class="collapseButton" type="button" aria-label={`${expanded ? copy.collapse : copy.expand} ${title}`} aria-controls={id} aria-expanded={expanded} on:click={() => expanded = !expanded}><span class:closed={!expanded} aria-hidden="true">❯</span></button>{/if}</div>
  </div>
  <div id={id} class="panelBody" hidden={!expanded}><slot /></div>
</section>
<style>
  .otcPanel { min-width: 0; border: 1px solid var(--color-border-strong); border-radius: 10px; background: var(--exchange-card-bg); overflow: hidden; }
  .panelHead { display: flex; align-items: center; justify-content: space-between; gap: 10px; min-height: 53px; padding: 12px 19px 7px; }
  .collapsed .panelHead { border-bottom-color: transparent; }
  .panelTitle, .panelActions { display: flex; align-items: center; gap: 10px; min-width: 0; }
  h2 { font-size: 16px; font-weight: 600; letter-spacing: -.02em; }
  .panelTitle > span { font-family: var(--font-mono); font-size: 10px; color: var(--color-text-soft); }
  .collapseButton { display: grid; width: 29px; height: 29px; place-items: center; border-radius: 7px; color: var(--color-text-soft); }
  .collapseButton:hover { background: var(--color-panel); color: var(--color-text); }
  .collapseButton span { font-size: 21px; line-height: 1; -webkit-text-stroke: .55px currentColor; transition: transform .26s ease; transform: rotate(-90deg); }
  .collapseButton span.closed { transform: rotate(90deg); }
  .panelBody { min-width: 0; }
  @media (max-width: 640px) { .otcPanel { border-radius: 13px; } .panelHead { padding-inline: 16px; } }
  @media (pointer: coarse) { .collapseButton { width: 36px; height: 36px; } }
  @media (prefers-reduced-motion: reduce) { .collapseButton span { transition: none; } }
</style>
