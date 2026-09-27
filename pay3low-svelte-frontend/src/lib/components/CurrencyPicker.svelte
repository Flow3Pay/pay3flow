<script lang="ts">
  import { afterUpdate, onDestroy } from "svelte";
  import { CRYPTO_ASSETS } from "$lib/payment-methods";

  export let open: boolean;
  export let selected: string;
  export let onClose: () => void;
  export let onSelect: (currency: string) => void;

  const choices = [
    { id: "USD", name: "US dollar", mark: "$", color: "#168451" },
    { id: "RUB", name: "Russian ruble", mark: "₽", color: "#21a038" },
    { id: "AMD", name: "Armenian dram", mark: "֏", color: "#6d2c91" },
    ...CRYPTO_ASSETS.map(([id, name, color]) => ({ id, name, mark: id, color })),
  ];

  let wasOpen = false;
  let previousOverflow = "";
  let previousOverscrollBehavior = "";
  let dialog: HTMLDivElement;
  let dragging = false;
  let dragStartY = 0;
  let dragDistance = 0;

  function close() {
    onClose();
  }

  function onKeyDown(event: KeyboardEvent) {
    if (event.key === "Escape") close();
  }

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
    if (shouldClose) close();
    else dialog?.style.removeProperty("--sheet-drag");
  }

  afterUpdate(() => {
    if (open === wasOpen) return;
    wasOpen = open;
    if (open) {
      previousOverflow = document.body.style.overflow;
      previousOverscrollBehavior = document.body.style.overscrollBehavior;
      document.body.style.overflow = "hidden";
      document.body.style.overscrollBehavior = "none";
      window.addEventListener("keydown", onKeyDown);
    } else {
      document.body.style.overflow = previousOverflow;
      document.body.style.overscrollBehavior = previousOverscrollBehavior;
      window.removeEventListener("keydown", onKeyDown);
    }
  });

  onDestroy(() => {
    if (typeof document !== "undefined" && wasOpen) {
      document.body.style.overflow = previousOverflow;
      document.body.style.overscrollBehavior = previousOverscrollBehavior;
    }
    if (typeof window !== "undefined") window.removeEventListener("keydown", onKeyDown);
  });
</script>

{#if open}
  <div class="backdrop" on:mousedown={close} role="presentation">
    <div class:dragging class="dialog" bind:this={dialog} role="dialog" aria-modal="true" aria-label="Choose currency" tabindex="-1" on:mousedown|stopPropagation>
      <button type="button" class="sheetHandle" aria-label="Close currency picker by dragging down" on:pointerdown={startSheetDrag} on:pointermove={moveSheetDrag} on:pointerup={endSheetDrag} on:pointercancel={endSheetDrag}><span aria-hidden="true"></span></button>
      <div class="titleBar">
        <div class="titleGroup">
          <button type="button" class="backButton" on:click={close} aria-label="Close currency picker">
            <svg width="21" height="21" viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="m7 7 10 10m0-10L7 17" stroke="currentColor" stroke-width="2" stroke-linecap="round" /></svg>
          </button>
          <h2 class="title">Choose currency</h2>
        </div>
      </div>
      <div class="body" role="listbox" aria-label="Currencies">
        {#each choices as choice}
          {@const isSelected = choice.id === selected}
          <button type="button" role="option" aria-selected={isSelected} class="currencyRow" data-selected={isSelected || undefined} on:click={() => onSelect(choice.id)}>
            <span class="currencyMark" style:background-color={choice.color} aria-hidden="true">{choice.mark}</span>
            <span class="currencyCopy"><strong>{choice.id}</strong><small>{choice.name}</small></span>
            {#if isSelected}<svg class="check" width="20" height="20" viewBox="0 0 20 20" fill="none" aria-hidden="true"><circle cx="10" cy="10" r="10" fill="currentColor" /><path d="m6 10.2 2.7 2.5 5.3-5.6" stroke="white" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" /></svg>{/if}
          </button>
        {/each}
      </div>
    </div>
  </div>
{/if}

<style>
.backdrop { position: fixed; inset: 0; z-index: 1100; display: grid; place-items: center; padding: 24px; background: rgba(15, 17, 14, 0.66); backdrop-filter: blur(18px) saturate(120%); -webkit-backdrop-filter: blur(18px) saturate(120%); animation: backdropIn 0.2s ease-out; touch-action: none; }
.dialog { width: min(100%, 520px); overflow: hidden; border: 1px solid rgba(255, 255, 255, 0.72); border-radius: 30px; background: rgba(250, 250, 246, 0.98); box-shadow: 0 38px 120px rgba(0, 0, 0, 0.35); animation: dialogIn 0.28s cubic-bezier(0.22, 1, 0.36, 1); touch-action: auto; }
.titleBar, .titleGroup, .currencyRow { display: flex; align-items: center; }
.titleBar { min-height: 78px; padding: 15px 21px; border-bottom: 1px solid var(--color-border); }
.titleGroup { gap: 10px; }
.title { margin: 0; font-size: 18px; font-weight: 750; letter-spacing: -0.035em; }
.backButton { display: grid; width: 42px; height: 42px; place-items: center; border-radius: 13px; color: var(--color-text-soft); }
.backButton:hover { background: var(--color-panel); }
.sheetHandle { display: none; }
.body { display: grid; max-height: min(640px, 70vh); gap: 8px; overflow-y: auto; padding: 18px; }
.currencyRow { width: 100%; gap: 14px; padding: 14px; border: 1px solid transparent; border-radius: 18px; color: var(--color-text); text-align: left; }
.currencyRow:hover, .currencyRow[data-selected="true"] { border-color: var(--color-border-strong); background: var(--color-panel); }
.currencyMark { display: grid; width: 48px; height: 48px; flex: 0 0 48px; place-items: center; border-radius: 15px; color: white; font-size: 23px; font-weight: 800; }
.currencyCopy { display: grid; gap: 3px; flex: 1; }
.currencyCopy strong { font-size: 15px; }
.currencyCopy small { color: var(--color-text-soft); font-size: 12px; }
.check { color: var(--color-violet); }
:global(html[data-theme="dark"]) .dialog { border-color: #3b3b3b; background: rgba(25, 25, 25, 0.98); }
:global(html[data-theme="dark"]) .currencyRow:hover,
:global(html[data-theme="dark"]) .currencyRow[data-selected="true"] { background: #2a2a2a; }
@media (max-width: 640px) { .backdrop { align-items: end; padding: 0; } .dialog { width: 100%; border-radius: 28px 28px 0 0; transform: translateY(var(--sheet-drag, 0)); } .sheetHandle { display: grid; width: 100%; height: 28px; place-items: center; } .sheetHandle span { width: 40px; height: 4px; border-radius: 999px; background: var(--color-border-strong); } }
@keyframes backdropIn { from { opacity: 0; } to { opacity: 1; } }
@keyframes dialogIn { from { opacity: 0; transform: translateY(14px) scale(0.985); } to { opacity: 1; transform: translateY(0) scale(1); } }
</style>
