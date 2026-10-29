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
<div class="marketPicker">
  <label class="pickerLabel" for="otc-market">{$locale === "ru" ? "Рынок OTC" : "OTC market"}</label>
  <span class="pickerShell">
    <span class="marketChip" aria-hidden="true"><img src={market === "GNK-USDT" ? "/icons/assets/gnk.jpg" : "/icons/assets/ever.svg"} width="22" height="22" alt="" /><strong>{market === "GNK-USDT" ? "GNK / USDT" : "EVER / USDT"}</strong><span>{market === "GNK-USDT" ? "HEX" : "Pay3Flow"}</span><svg width="12" height="12" viewBox="0 0 16 16" fill="none"><path d="m4 6 4 4 4-4" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg></span>
    <select id="otc-market" value={market} on:change={choose}><option value="EVER-USDT">EVER / USDT · Pay3Flow</option><option value="GNK-USDT">GNK / USDT · HEX</option></select>
  </span>
</div>
{#if Workspace}<svelte:component this={Workspace} {onPreviewChange} {onShareUrlChange} />{/if}
<style>
  .marketPicker { display:flex; align-items:center; justify-content:flex-end; gap:12px; width:min(var(--layout-width),calc(100% - 2 * var(--page-gutter))); margin:20px auto 0; font-size:13px; }
  .pickerLabel { position:absolute; width:1px; height:1px; padding:0; margin:-1px; overflow:hidden; clip:rect(0 0 0 0); clip-path:inset(50%); white-space:nowrap; border:0; }
  .pickerShell { position:relative; display:flex; min-width:0; }
  .marketChip { display:flex; min-width:0; align-items:center; gap:8px; padding:8px 12px; border:1px solid var(--color-border); border-radius:var(--radius-pill); background:var(--color-paper); font-size:13px; }
  .marketChip img { flex:0 0 22px; border-radius:50%; }
  .marketChip strong { font-family:var(--font-mono); font-size:12px; font-weight:600; letter-spacing:.02em; white-space:nowrap; }
  .marketChip > span { color:var(--color-text-soft); font-size:12px; white-space:nowrap; }
  .marketChip svg { flex:0 0 auto; margin-left:2px; color:var(--color-text-soft); }
  select { position:absolute; inset:0; width:100%; height:100%; margin:0; padding:0; border:0; opacity:0; cursor:pointer; font:inherit; }
  .pickerShell:focus-within .marketChip { outline:var(--focus-ring-width) solid var(--color-focus); outline-offset:2px; }
</style>
