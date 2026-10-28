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
  .otcPanel { display: flex; flex: 1; flex-direction: column; width: 100%; min-width: 0; border: 1px solid var(--color-border-strong); border-radius: var(--otc-card-radius, 24px); background: var(--exchange-card-bg); overflow: hidden; }
  .panelHead { display: flex; flex: 0 0 auto; min-height: 56px; align-items: center; justify-content: space-between; gap: 12px; padding: 16px var(--otc-panel-padding, 20px); }
  .panelTitle, .panelActions { display: flex; min-width: 0; align-items: center; gap: 8px; }
  .panelActions { flex: 0 0 auto; }
  h2 { margin: 0; font-size: 16px; font-weight: 650; letter-spacing: -.02em; line-height: 1.5; overflow-wrap: anywhere; }
  .panelTitle > span { color: var(--color-text-soft); font-family: var(--font-mono); font-size: 12px; }
  .collapseButton { display: grid; width: 44px; height: 44px; place-items: center; border-radius: 8px; color: var(--color-text-soft); }
  .collapseButton:hover { background: var(--color-panel); color: var(--color-text); }
  .collapseButton span { font-size: 20px; line-height: 1; transition: transform .24s ease; transform: rotate(-90deg); }
  .collapseButton span.closed { transform: rotate(90deg); }
  .panelBody { display: flex; flex: 1; flex-direction: column; min-width: 0; }
  .panelBody[hidden] { display: none; }
  @media (prefers-reduced-motion: reduce) { .collapseButton span { transition: none; } }
</style>
