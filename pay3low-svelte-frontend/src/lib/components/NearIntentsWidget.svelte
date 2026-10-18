<script lang="ts">
  import { onMount } from "svelte";
  import { env } from "$env/dynamic/public";
  import type { RouteCandidate } from "$lib/exchange";
  import type { TutorialStep } from "$lib/route-tutorial";
  import type { MountedNearWidget } from "$lib/near-widget";
  import { locale, t } from "$lib/i18n";

  export let route: RouteCandidate;
  export let step: TutorialStep;
  export let networkNames: Record<string, string> = {};
  let container: HTMLDivElement;
  let ready = false;
  let error = false;
  $: copy = (key: string) => t(key, {}, $locale);

  onMount(() => {
    let disposed = false;
    let widget: MountedNearWidget | undefined;
    const theme = () => document.documentElement.dataset.theme === "dark" ? "dark" : "light";
    const observer = new MutationObserver(() => widget?.updateTheme(theme()));
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    void import("$lib/near-widget").then(({ mountNearWidget }) => {
      if (disposed) return;
      widget = mountNearWidget(container, route, step, networkNames, env.PUBLIC_AURORA_INTENTS_API_KEY, theme());
      ready = true;
    }).catch(() => { if (!disposed) error = true; });
    return () => { disposed = true; observer.disconnect(); widget?.destroy(); };
  });
</script>

<section class="nearIntentsRoot" data-testid="near-intents-live-widget" aria-label="NEAR Intents">
  {#if !ready && !error}<p role="status">{copy("Loading NEAR Intents…")}</p>{/if}
  {#if error}<p role="alert">{copy("Could not load NEAR Intents. Try again.")}</p>{/if}
  <div bind:this={container}></div>
</section>

<style>
  .nearIntentsRoot { width: min(100%, 560px); min-width: 0; margin: 24px auto 0; }
  p { color: var(--color-text-soft); font-size: 12px; }
  p[role="alert"] { color: var(--color-danger); }
</style>
