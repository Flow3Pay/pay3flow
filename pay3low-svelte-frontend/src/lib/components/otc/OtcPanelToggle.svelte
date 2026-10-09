<script lang="ts">
  import { locale } from "$lib/i18n";
  import { otcCopy } from "$lib/otc/copy";
  export let title: string;
  export let controls: string;
  export let expanded = true;
  export let vertical = false;
  export let direction: "left" | "right" = "left";
  $: label = `${expanded ? otcCopy($locale).collapse : otcCopy($locale).expand} ${title}`;
</script>

<button type="button" class="panelToggle" class:vertical class:right={direction === "right"} class:expanded aria-label={label} title={label} aria-controls={controls} aria-expanded={expanded} on:click={() => expanded = !expanded}><span aria-hidden="true">❯</span></button>

<style>
  .panelToggle { display: grid; width: 32px; min-width: 32px; height: 42px; place-items: center; padding: 0; color: var(--color-text-soft); }
  .panelToggle:hover { color: var(--color-text); }
  span { display: block; font-size: 21px; line-height: 1; -webkit-text-stroke: .55px currentColor; transform: rotate(180deg); transition: transform .3s cubic-bezier(.22, 1, .36, 1); }
  .expanded span { transform: rotate(0); }
  .right span { transform: rotate(0); }
  .right.expanded span { transform: rotate(180deg); }
  .vertical span { transform: rotate(90deg); }
  .vertical.expanded span { transform: rotate(-90deg); }
  @media (max-width: 980px) { .panelToggle { width: 48px; } span { transform: rotate(-90deg); } .expanded span { transform: rotate(90deg); } .right span { transform: rotate(90deg); } .right.expanded span { transform: rotate(-90deg); } }
  @media (prefers-reduced-motion: reduce) { span { transition: none; } }
</style>
