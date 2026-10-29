<script lang="ts">
  import { afterUpdate, onDestroy } from "svelte";
  import { locale } from "$lib/i18n";
  import { lockPageScroll } from "$lib/page-scroll-lock";
  import { navigatePicker } from "$lib/picker-keyboard";
  import { otcMarkets, type OtcMarket } from "$lib/otc/markets";
  import PickerOptionCard from "../PickerOptionCard.svelte";

  export let open = false;
  export let selected = "";
  export let onClose: () => void;
  export let onSelect: (market: OtcMarket) => void;

  let query = "";
  let input: HTMLInputElement;
  let dialog: HTMLDivElement;
  let wasOpen = false;
  let unlockPage: (() => void) | undefined;
  let focusTimer: number | undefined;
  let dragging = false;
  let dragStartY = 0;
  let dragDistance = 0;

  $: say = (en: string, ru: string) => $locale === "ru" ? ru : en;
  const normalizeSearch = (value: string) => value.normalize("NFKC").toLocaleLowerCase().replace(/[^\p{L}\p{N}]+/gu, " ").trim();
  const searchMatches = (text: string, queryValue: string) => {
    const normalized = normalizeSearch(text);
    const compact = normalized.replaceAll(" ", "");
    return normalizeSearch(queryValue).split(" ").filter(Boolean).every((term) => normalized.includes(term) || compact.includes(term));
  };

  $: visibleMarkets = normalizeSearch(query)
    ? otcMarkets.filter((market) => searchMatches(`${market.base} ${market.quote} ${market.id} ${market.network} ${market.venue} ${market.keywords}`, query))
    : otcMarkets;

  function close() { query = ""; onClose(); }
  function choose(market: OtcMarket) { query = ""; onSelect(market); }
  function startSheetDrag(event: PointerEvent) { dragging = true; dragStartY = event.clientY; dragDistance = 0; (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId); }
  function moveSheetDrag(event: PointerEvent) { if (!dragging) return; dragDistance = Math.max(0, event.clientY - dragStartY); dialog?.style.setProperty("--sheet-drag", `${dragDistance}px`); }
  function endSheetDrag() { if (!dragging) return; const shouldClose = dragDistance > 96 || (dialog && dragDistance > dialog.clientHeight * .24); dragging = false; if (shouldClose) close(); else dialog?.style.removeProperty("--sheet-drag"); }
  function onKeyDown(event: KeyboardEvent) { if (event.key === "Escape") close(); else navigatePicker(event, dialog); }

  afterUpdate(() => {
    if (open === wasOpen) return;
    wasOpen = open;
    if (open) {
      query = "";
      unlockPage = lockPageScroll(); window.addEventListener("keydown", onKeyDown);
      focusTimer = window.setTimeout(() => input?.focus(), 80);
    } else {
      unlockPage?.(); unlockPage = undefined; window.removeEventListener("keydown", onKeyDown); if (focusTimer) window.clearTimeout(focusTimer);
    }
  });
  onDestroy(() => { unlockPage?.(); if (typeof window !== "undefined") window.removeEventListener("keydown", onKeyDown); if (typeof window !== "undefined" && focusTimer) window.clearTimeout(focusTimer); });
</script>

{#if open}
  <div class="backdrop" on:mousedown={close} role="presentation">
    <div class:dragging class="dialog" bind:this={dialog} role="dialog" aria-modal="true" aria-label={say("Choose an OTC market", "Выберите рынок OTC")} tabindex="-1" on:mousedown|stopPropagation>
      <button type="button" class="sheetHandle" aria-label={say("Close OTC market picker by dragging down", "Закрыть выбор рынка OTC смахиванием вниз")} on:pointerdown={startSheetDrag} on:pointermove={moveSheetDrag} on:pointerup={endSheetDrag} on:pointercancel={endSheetDrag}><span aria-hidden="true"></span></button>
      <div class="searchRow">
        <label class="searchBox"><svg width="18" height="18" viewBox="0 0 24 24" fill="none" aria-hidden="true"><circle cx="11" cy="11" r="7" stroke="currentColor" stroke-width="1.7" /><path d="m20 20-4-4" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" /></svg><input bind:this={input} bind:value={query} placeholder={say("Search markets", "Поиск рынков")} aria-label={say("OTC markets", "Рынки OTC")} /></label>
        <button type="button" class="closeButton" on:click={close} aria-label={say("Close OTC market picker", "Закрыть выбор рынка OTC")}><kbd>esc</kbd><svg width="17" height="17" viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="m7 7 10 10m0-10L7 17" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" /></svg></button>
      </div>
      <div class="titleBar"><h2 class="title">{say("Choose an OTC market", "Выберите рынок OTC")}</h2></div>
      <section class="resultPanel">
        <div class="methods" role="listbox" aria-label={say("OTC markets", "Рынки OTC")}>
          {#if visibleMarkets.length === 0}
            <div class="empty"><strong>{say("No markets found", "Рынки не найдены")}</strong><span>{say("Try a different search.", "Попробуйте другой запрос.")}</span></div>
          {:else}
            <section class="section">
              <h3>{say("Markets", "Рынки")}</h3>
              {#each visibleMarkets as market (market.id)}
                <PickerOptionCard name={`${market.base} / ${market.quote}`} meta={market.network} category={market.venue} iconUrl={market.icon} initials={market.base} color="#eef2ea" selected={selected === market.id} ariaLabel={`${market.base} / ${market.quote} · ${market.network}`} onSelect={() => choose(market)} />
              {/each}
            </section>
          {/if}
        </div>
      </section>
    </div>
  </div>
{/if}

<style>
  .backdrop { position: fixed; inset: 0; z-index: 1100; display: grid; place-items: center; padding: 24px; background: #11172255; backdrop-filter: blur(5px); touch-action: none; }
  .dialog { display: flex; flex-direction: column; width: min(100%, 480px); max-height: min(620px, 88dvh); overflow: hidden; border: 1px solid var(--color-border); border-radius: 12px; background: var(--color-paper); box-shadow: 0 24px 90px #11172224, 0 3px 12px #11172212; touch-action: auto; }
  .searchRow { display: flex; align-items: center; gap: 6px; flex: 0 0 auto; min-height: 58px; padding: 6px 10px 6px 16px; border-bottom: 1px solid var(--color-border); }
  .searchBox { display: flex; align-items: center; gap: 10px; min-width: 0; flex: 1; min-height: 44px; color: var(--color-text-faint); }
  .searchBox svg { flex: 0 0 auto; }.searchBox input { width: 100%; min-width: 0; border: 0; padding: 0; outline: 0; background: transparent; color: var(--color-text); font: inherit; font-size: 14px; font-weight: 450; }.searchBox input::placeholder { color: var(--color-text-faint); opacity: 1; }
  .titleBar { display: flex; align-items: center; gap: 8px; justify-content: space-between; padding: 10px 16px 2px; }.title { margin: 0; font-size: 11px; font-weight: 500; color: var(--color-text-soft); }
  .closeButton { display: grid; place-items: center; flex: 0 0 44px; min-height: 44px; padding: 0; border-radius: 6px; color: var(--color-text-faint); }.closeButton:hover { background: var(--color-panel-soft); }.closeButton svg { display: none; }kbd { font: 10px var(--font-mono); border: 1px solid var(--color-border); padding: 3px 5px; border-radius: 4px; color: var(--color-text-faint); }
  .resultPanel { display: flex; flex-direction: column; min-height: 0; overflow: hidden; flex: 1; }.methods { min-height: 0; max-height: min(480px, calc(88dvh - 106px)); overflow-y: auto; overscroll-behavior: contain; padding: 4px 8px 12px; scrollbar-gutter: stable; }.section { padding: 0 0 8px; }.section h3 { margin: 0; padding: 7px 8px; color: var(--color-text-faint); font-size: 10px; font-weight: 500; }.empty { display: grid; gap: 6px; padding: 28px 12px; color: var(--color-text-soft); text-align: center; font-size: 12px; }.empty strong { font-size: 13px; font-weight: 600; }.empty span { color: var(--color-text-faint); }
  .sheetHandle { display: none; }
  @media (max-width: 700px) { .backdrop { align-items: end; padding: 0; }.dialog { width: 100%; max-height: 86dvh; border-radius: 16px 16px 0 0; transform: translateY(var(--sheet-drag, 0)); transition: transform .24s ease; padding-bottom: env(safe-area-inset-bottom); }.dialog.dragging { transition: none; }.sheetHandle { display: flex; min-height: 30px; align-items: center; justify-content: center; width: 100%; padding: 0; color: var(--color-text-faint); }.sheetHandle span { width: 36px; height: 4px; background: currentColor; border-radius: 99px; }.searchRow { min-height: 56px; padding-left: 14px; }.searchBox input { font-size: 16px; }.methods { max-height: calc(86dvh - 135px); padding-bottom: 18px; }.closeButton kbd { display: none; }.closeButton svg { display: block; } }
  @media (prefers-reduced-motion: reduce) { .dialog { transition: none; } }
</style>
