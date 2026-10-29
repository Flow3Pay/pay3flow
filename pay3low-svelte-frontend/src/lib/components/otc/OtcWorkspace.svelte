<script lang="ts">
  import { onMount } from "svelte";
  import type { OtcPreview } from "$lib/share-links";
  import { parseOtcHash } from "$lib/otc/link";
  import type OtcEverWorkspace from "./OtcEverWorkspace.svelte";
  let Workspace: typeof OtcEverWorkspace | null = null;
  let version = 0;
  export let onPreviewChange: (preview: OtcPreview | null) => void = () => {};
  export let onShareUrlChange: (url: string) => void = () => {};
  export let introCompleted = true;
  let market = "", ready = false;
  onMount(() => {
    const sync = async () => {
      const next = parseOtcHash(location.hash).marketId === "GNK-USDT" ? "GNK-USDT" : "EVER-USDT";
      if (ready && next === market && Workspace) return;
      if (ready && next !== market) onPreviewChange(null);
      market = next; ready = true; Workspace = null; const current = ++version;
      const component = next === "GNK-USDT" ? (await import("./OtcGonkaWorkspace.svelte")).default : (await import("./OtcEverWorkspace.svelte")).default;
      if (current === version) Workspace = component;
    };
    sync(); window.addEventListener("hashchange",sync); window.addEventListener("popstate",sync);
    return () => { version++; window.removeEventListener("hashchange",sync); window.removeEventListener("popstate",sync); };
  });
</script>
{#if Workspace}<svelte:component this={Workspace} {onPreviewChange} {onShareUrlChange} {introCompleted} />{/if}
