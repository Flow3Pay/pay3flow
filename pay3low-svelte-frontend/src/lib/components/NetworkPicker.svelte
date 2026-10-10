<script lang="ts">
  import { afterUpdate, onDestroy } from "svelte";
  import type { CryptoNetwork } from "$lib/networks";
  import { networkIcon } from "$lib/icons";
  import { locale, t } from "$lib/i18n";
  import { fiatFlagUrl } from "$lib/currency-flags";
  import { lockPageScroll } from "$lib/page-scroll-lock";
  import PickerOptionCard from "./PickerOptionCard.svelte";
  import { navigatePicker } from "$lib/picker-keyboard";

  type CurrencyChoice = {
    id: string;
    name: string;
    mark: string;
    color: string;
  };

  export let open: boolean;
  export let mode: "network" | "currency" = "network";
  export let networks: CryptoNetwork[] = [];
  export let selected: CryptoNetwork | undefined = undefined;
  export let currencies: CurrencyChoice[] = [];
  export let selectedCurrency = "";
  export let onClose: () => void;
  export let onSelect: (network: CryptoNetwork) => void = () => {};
  export let onSelectCurrency: (currency: string) => void = () => {};
  let wasOpen = false;
  let unlockPage: (() => void) | undefined;
  let dialog: HTMLDivElement;
  let dragging = false;
  let dragStartY = 0;
  let dragDistance = 0;
  let query = "";
  let input: HTMLInputElement;
  let focusTimer: number | undefined;
  const onKeyDown = (event: KeyboardEvent) => { if (event.key === "Escape") onClose(); else navigatePicker(event, dialog); };
  const matches = (text: string, search: string) => search.normalize("NFKC").toLocaleLowerCase().replace(/[^\p{L}\p{N}]+/gu, " ").trim().split(" ").filter(Boolean).every(term => text.normalize("NFKC").toLocaleLowerCase().replace(/[^\p{L}\p{N}]+/gu, "").includes(term));
  $: visibleCurrencies = currencies.filter(currency => matches(`${currency.id} ${currency.name}`, query));
  $: visibleNetworks = networks.filter(network => matches(`${network.id} ${network.name}`, query));

  function startSheetDrag(event: PointerEvent) {
    dragging = true;
    dragStartY = event.clientY;
    dragDistance = 0;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  }

  function moveSheetDrag(event: PointerEvent) {
    if (!dragging) return;
    dragDistance = Math.max(0, event.clientY - dragStartY);
    dialog?.style.setProperty("--sheet-drag", `${dragDistance}px`);
  }

  function endSheetDrag() {
    if (!dragging) return;
    const shouldClose = dragDistance > 96 || (dialog && dragDistance > dialog.clientHeight * 0.24);
    dragging = false;
    if (shouldClose) {
      onClose();
    } else {
      dialog?.style.removeProperty("--sheet-drag");
    }
  }

  afterUpdate(() => {
    if (open === wasOpen) return;
    wasOpen = open;
    if (open) {
      query = "";
      focusTimer = window.setTimeout(() => input?.focus(), 80);
      unlockPage = lockPageScroll();
      window.addEventListener("keydown", onKeyDown);
    } else {
      if (focusTimer) window.clearTimeout(focusTimer);
      unlockPage?.();
      unlockPage = undefined;
      window.removeEventListener("keydown", onKeyDown);
    }
  });
  onDestroy(() => {
    unlockPage?.();
    if (typeof window !== "undefined") window.removeEventListener("keydown", onKeyDown);
    if (typeof window !== "undefined" && focusTimer) window.clearTimeout(focusTimer);
  });
</script>

{#if open}
  <div class="backdrop" on:mousedown={onClose} role="presentation">
    <div class:dragging class="dialog" bind:this={dialog} role="dialog" aria-modal="true" aria-label={t(mode === "currency" ? "Choose currency" : "Choose network", {}, $locale)} tabindex="-1" on:mousedown|stopPropagation>
      <button type="button" class="sheetHandle" aria-label={t(mode === "currency" ? "Close currency picker by dragging down" : "Close network picker by dragging down", {}, $locale)} on:pointerdown={startSheetDrag} on:pointermove={moveSheetDrag} on:pointerup={endSheetDrag} on:pointercancel={endSheetDrag}>
        <span aria-hidden="true"></span>
      </button>
      <div class="searchRow"><label class="searchBox"><svg width="18" height="18" viewBox="0 0 24 24" fill="none" aria-hidden="true"><circle cx="11" cy="11" r="7" stroke="currentColor" stroke-width="1.7" /><path d="m20 20-4-4" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" /></svg><input bind:this={input} bind:value={query} placeholder={t(mode === "currency" ? "Currency or digital asset" : "Search blockchains", {}, $locale)} aria-label={t(mode === "currency" ? "Currencies and digital assets" : "Blockchains", {}, $locale)} /></label><button type="button" class="backButton" on:click={onClose} aria-label={t(mode === "currency" ? "Close currency picker" : "Close network picker", {}, $locale)}><kbd>esc</kbd><svg width="17" height="17" viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="m7 7 10 10m0-10L7 17" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" /></svg></button></div>
      <div class="titleBar"><div class="titleGroup">
        <h2 class="title">{t(mode === "currency" ? "Choose currency" : "Choose network", {}, $locale)}</h2>
      </div></div>
      <div class="body"><div class="methods" role="listbox" aria-label={t(mode === "currency" ? "Currencies" : "Crypto networks", {}, $locale)}><section class="section">
        <h3>{t(mode === "currency" ? "Available currencies" : "Available networks", {}, $locale)}</h3>
        {#if mode === "currency"}
          {#each visibleCurrencies as currency (currency.id)}
            {@const isSelected = currency.id === selectedCurrency}
            <PickerOptionCard name={currency.id} meta={currency.name} category={t("Currencies", {}, $locale)} iconUrl={fiatFlagUrl(currency.id)} initials={currency.mark} color={currency.color} selected={isSelected} onSelect={() => onSelectCurrency(currency.id)} />
          {:else}<div class="empty">{t("Try a different search.", {}, $locale)}</div>{/each}
        {:else}
          {#each visibleNetworks as network (network.id)}
            {@const isSelected = network.id === selected?.id}
            <PickerOptionCard name={network.name} category={t("Network", {}, $locale)} iconUrl={networkIcon(network.name)} initials={network.name.slice(0, 2).toUpperCase()} color="#eef2ea" selected={isSelected} onSelect={() => onSelect(network)} />
          {:else}<div class="empty">{t("No compatible blockchains found", {}, $locale)}</div>{/each}
        {/if}
      </section></div></div>
    </div>
  </div>
{/if}

<style>
  .backdrop { position: fixed; inset: 0; z-index: 1100; display: grid; place-items: center; padding: 24px; background: #11172255; backdrop-filter: blur(5px); touch-action: none; }
  .dialog { display: flex; flex-direction: column; width: min(100%, 480px); max-height: min(620px, 88dvh); overflow: hidden; border: 1px solid var(--color-border); border-radius: 12px; background: var(--color-paper); box-shadow: 0 24px 90px #11172224, 0 3px 12px #11172212; touch-action: auto; }
  .searchRow { display: flex; align-items: center; gap: 6px; flex: 0 0 auto; min-height: 58px; padding: 6px 10px 6px 16px; border-bottom: 1px solid var(--color-border); }
  .searchBox { display: flex; align-items: center; gap: 10px; min-width: 0; flex: 1; min-height: 44px; color: var(--color-text-faint); }
  .searchBox svg { flex: 0 0 auto; }.searchBox input { width: 100%; min-width: 0; border: 0; padding: 0; outline: 0; background: transparent; color: var(--color-text); font: inherit; font-size: 14px; font-weight: 450; }.searchBox input::placeholder { color: var(--color-text-faint); opacity: 1; }
  .searchRow:focus-within { box-shadow: inset 0 -2px var(--color-focus); }
  .titleBar { display: flex; align-items: center; gap: 8px; justify-content: space-between; padding: 10px 16px 2px; }.titleGroup { min-width: 0; }.title { margin: 0; font-size: 11px; font-weight: 500; color: var(--color-text-soft); }
  .backButton { display: grid; place-items: center; flex: 0 0 44px; min-height: 44px; padding: 0; border-radius: 6px; color: var(--color-text-faint); }.backButton:hover { background: var(--color-panel-soft); }.backButton svg { display: none; }kbd { font: 10px var(--font-mono); border: 1px solid var(--color-border); padding: 3px 5px; border-radius: 4px; color: var(--color-text-faint); }
  .body { display: flex; flex-direction: column; min-height: 0; overflow: hidden; flex: 1; }.methods { min-height: 0; max-height: min(480px, calc(88dvh - 106px)); overflow-y: auto; overscroll-behavior: contain; padding: 4px 8px 12px; scrollbar-gutter: stable; }.section { padding: 0 0 8px; }.section + .section { border-top: 1px solid var(--color-border); margin-top: 4px; padding-top: 7px; }.section h3 { margin: 0; padding: 7px 8px; color: var(--color-text-faint); font-size: 10px; font-weight: 500; }.empty { display: grid; gap: 6px; padding: 28px 12px; color: var(--color-text-soft); text-align: center; font-size: 12px; }.empty strong { font-size: 13px; font-weight: 600; }.empty span { color: var(--color-text-faint); }
  .sheetHandle { display: none; }
  @media (max-width: 700px) { .backdrop { align-items: end; padding: 0; }.dialog { width: 100%; max-height: 86dvh; border-radius: 16px 16px 0 0; transform: translateY(var(--sheet-drag, 0)); transition: transform .24s ease; padding-bottom: env(safe-area-inset-bottom); }.dialog.dragging { transition: none; }.sheetHandle { display: flex; min-height: 30px; align-items: center; justify-content: center; width: 100%; padding: 0; color: var(--color-text-faint); }.sheetHandle span { width: 36px; height: 4px; background: currentColor; border-radius: 99px; }.searchRow { min-height: 56px; padding-left: 14px; }.searchBox input { font-size: 16px; }.methods { max-height: calc(86dvh - 135px); padding-bottom: 18px; }.backButton kbd { display: none; }.backButton svg { display: block; } }
  @media (prefers-reduced-motion: reduce) { .dialog { transition: none; } }
</style>
