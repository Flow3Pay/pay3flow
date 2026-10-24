<script lang="ts">
  import { onMount } from "svelte";
  import { locale } from "$lib/i18n";
  import type { OtcPreview } from "$lib/share-links";
  import { parseOtcHash } from "$lib/otc/link";
  import type OtcEverWorkspace from "./OtcEverWorkspace.svelte";
  let Workspace: typeof OtcEverWorkspace | null = null;
  let version = 0;
  export let onPreviewChange: (preview: OtcPreview | null) => void = () => {};
  export let onShareUrlChange: (url: string) => void = () => {};
  let market = "EVER-USDT", ready = false;
  onMount(() => {
    const sync = async () => {
      const next = parseOtcHash(location.hash).marketId === "GNK-USDT" ? "GNK-USDT" : "EVER-USDT";
      if (ready && next === market && Workspace) return;
      market = next; ready = true; Workspace = null; const current = ++version;
      const component = next === "GNK-USDT" ? (await import("./OtcGonkaWorkspace.svelte")).default : (await import("./OtcEverWorkspace.svelte")).default;
      if (current === version) Workspace = component;
    };
    sync(); window.addEventListener("hashchange",sync); window.addEventListener("popstate",sync);
    return () => { version++; window.removeEventListener("hashchange",sync); window.removeEventListener("popstate",sync); };
  });
  function choose(event: Event) {
    const next = (event.currentTarget as HTMLSelectElement).value;
    Workspace = null; ready = false; market = next;
    onPreviewChange(null); location.hash = `#/otc?market=${market}`;
  }
</script>
<div class="marketPicker"><label for="otc-market">{$locale === "ru" ? "Рынок OTC" : "OTC market"}</label><select id="otc-market" value={market} on:change={choose}><option value="EVER-USDT">EVER / USDT · Pay3Flow</option><option value="GNK-USDT">GNK / USDT · HEX</option></select></div>
{#if Workspace}<svelte:component this={Workspace} {onPreviewChange} {onShareUrlChange} />{/if}
<style>
  .marketPicker { display:flex; align-items:center; justify-content:flex-end; gap:12px; width:min(var(--layout-width),calc(100% - 2 * var(--page-gutter))); margin:24px auto 0; font-size:13px; }
  label { color:var(--color-text-soft); }
  select { min-height:44px; max-width:75%; padding:8px 12px; border:1px solid var(--color-border); border-radius:8px; background:var(--color-paper); color:var(--color-text); font:inherit; }
</style>
