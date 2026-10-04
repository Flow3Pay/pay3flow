<script lang="ts">
  import { afterUpdate, onDestroy } from "svelte";
  import { paymentMethodFavicon, type PaymentMethod } from "$lib/payment-methods";
  import { networkIcon } from "$lib/icons";
  import type { CryptoNetwork } from "$lib/networks";
  import { locale, t } from "$lib/i18n";
  import { lockPageScroll } from "$lib/page-scroll-lock";
  import PickerOptionCard from "./PickerOptionCard.svelte";

  export let open: boolean;
  export let title: string;
  export let role: "sender" | "recipient";
  export let networks: CryptoNetwork[];
  export let paymentMethods: PaymentMethod[];
  export let selected: PaymentMethod | null;
  export let selectedNetwork: CryptoNetwork | undefined;
  export let onClose: () => void;
  export let onSelect: (method: PaymentMethod, network?: CryptoNetwork) => void;

  type PickerStage = "currency" | "detail";
  type CurrencyChoice = { id: string; name: string; kind: "fiat" | "asset"; initials: string; color: string; iconUrl?: string };
  type FiatGroup = { method: PaymentMethod; variants: PaymentMethod[] };
  type Option = { method: PaymentMethod; network?: CryptoNetwork; variants?: PaymentMethod[] };

  let stage: PickerStage = "currency";
  let activeCurrency = "";
  let query = "";
  let input: HTMLInputElement;
  let dialog: HTMLDivElement;
  let wasOpen = false;
  let unlockPage: (() => void) | undefined;
  let focusTimer: number | undefined;
  let dragging = false;
  let dragStartY = 0;
  let dragDistance = 0;
  let detailOptions: Option[] = [];

  const normalizeSearch = (value: string) => value.normalize("NFKC").toLocaleLowerCase().replace(/[^\p{L}\p{N}]+/gu, " ").trim();
  const searchMatches = (text: string, queryValue: string) => {
    const normalized = normalizeSearch(text);
    const compact = normalized.replaceAll(" ", "");
    return normalizeSearch(queryValue).split(" ").filter(Boolean).every((term) => normalized.includes(term) || compact.includes(term));
  };
  const roleAllowed = (method: PaymentMethod) => method.role === role || method.role === "both";
  const currencyMatches = (method: PaymentMethod, currency: string) => method.currency.toUpperCase() === currency.toUpperCase();
  const methodLogo = (method: PaymentMethod, variants: PaymentMethod[] = [method]) => paymentMethodFavicon(method) ?? variants.map((item) => paymentMethodFavicon(item)).find(Boolean) ?? null;
  const methodMeta = (method: PaymentMethod, variants: PaymentMethod[] = [method]) => method.kind === "cash" ? `Cash settlement · ${method.currency}` : `Bank transfer · ${[...new Set(variants.map((item) => item.currency))].join(" / ")}`;

  function groupFiatMethods(methods: PaymentMethod[], current: PaymentMethod | null): FiatGroup[] {
    const groups = new Map<string, PaymentMethod[]>();
    for (const method of methods) {
      const key = method.currencyGroup ?? method.id;
      groups.set(key, [...(groups.get(key) ?? []), method]);
    }
    return [...groups.values()].map((variants) => ({ method: variants.find((item) => item.id === current?.id) ?? variants[0], variants }));
  }

  function close() { stage = "currency"; activeCurrency = ""; query = ""; onClose(); }
  function goBack() { stage = "currency"; activeCurrency = ""; query = ""; window.setTimeout(() => input?.focus(), 0); }
  function chooseCurrency(choice: CurrencyChoice) { stage = "detail"; activeCurrency = choice.id; query = ""; window.setTimeout(() => input?.focus(), 0); }
  function startSheetDrag(event: PointerEvent) { dragging = true; dragStartY = event.clientY; dragDistance = 0; (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId); }
  function moveSheetDrag(event: PointerEvent) { if (!dragging) return; dragDistance = Math.max(0, event.clientY - dragStartY); dialog?.style.setProperty("--sheet-drag", `${dragDistance}px`); }
  function endSheetDrag() { if (!dragging) return; const shouldClose = dragDistance > 96 || (dialog && dragDistance > dialog.clientHeight * .24); dragging = false; if (shouldClose) close(); else dialog?.style.removeProperty("--sheet-drag"); }
  function onKeyDown(event: KeyboardEvent) { if (event.key === "Escape") close(); }

  afterUpdate(() => {
    if (open === wasOpen) return;
    wasOpen = open;
    if (open) {
      stage = "currency"; activeCurrency = ""; query = "";
      unlockPage = lockPageScroll(); window.addEventListener("keydown", onKeyDown);
      focusTimer = window.setTimeout(() => input?.focus(), 80);
    } else {
      unlockPage?.(); unlockPage = undefined; window.removeEventListener("keydown", onKeyDown); if (focusTimer) window.clearTimeout(focusTimer);
    }
  });
  onDestroy(() => { unlockPage?.(); if (typeof window !== "undefined") window.removeEventListener("keydown", onKeyDown); if (typeof window !== "undefined" && focusTimer) window.clearTimeout(focusTimer); });

  $: fiatMethods = paymentMethods.filter((method) => (method.kind === "bank" || method.kind === "cash") && roleAllowed(method));
  $: assetMethods = paymentMethods.filter((method) => method.kind === "wallet" && roleAllowed(method));
  $: fiatChoices = [...new Set(fiatMethods.map((method) => method.currency.toUpperCase()))].map((currency): CurrencyChoice => { const catalog = paymentMethods.find((method) => method.kind === "currency" && currencyMatches(method, currency)); const source = fiatMethods.find((method) => currencyMatches(method, currency)); return { id: currency, name: catalog?.name ?? source?.name ?? currency, kind: "fiat", initials: catalog?.initials ?? currency.slice(0, 2), color: catalog?.color ?? source?.color ?? "#6d9800", iconUrl: catalog?.iconUrl }; }).sort((left, right) => left.id.localeCompare(right.id));
  $: assetChoices = [...new Map(assetMethods.map((method) => [method.currency.toUpperCase(), method])).values()].map((method): CurrencyChoice => ({ id: method.currency.toUpperCase(), name: method.name, kind: "asset", initials: method.initials || method.currency.slice(0, 2), color: method.color, iconUrl: paymentMethodFavicon(method) ?? undefined })).sort((left, right) => left.id.localeCompare(right.id));
  $: currencyChoices = [...fiatChoices, ...assetChoices];
  $: normalizedQuery = normalizeSearch(query);
  $: visibleCurrencies = normalizedQuery ? currencyChoices.filter((choice) => searchMatches(`${choice.id} ${choice.name}`, query)) : currencyChoices;
  $: selectedCurrency = selected?.currency?.toUpperCase() ?? "";
  $: activeChoice = currencyChoices.find((choice) => choice.id === activeCurrency);
  $: fiatGroups = activeChoice?.kind === "fiat" ? groupFiatMethods(fiatMethods.filter((method) => currencyMatches(method, activeCurrency)), selected) : [];
  $: assetMethod = activeChoice?.kind === "asset" ? assetMethods.find((method) => method.id === selected?.id && currencyMatches(method, activeCurrency)) ?? assetMethods.find((method) => currencyMatches(method, activeCurrency)) : undefined;
  $: assetNetworks = assetMethod ? networks.filter((network) => network.currencies.some((currency) => currency.toUpperCase() === activeCurrency)) : [];
  $: detailOptions = activeChoice?.kind === "fiat" ? fiatGroups.map(({ method, variants }) => ({ method, variants })) : assetMethod ? assetNetworks.map((network) => ({ method: assetMethod!, network })) : [];
  $: filteredDetails = normalizedQuery ? detailOptions.filter((option) => searchMatches([option.method.name, option.method.currency, option.method.p2pQuery, option.method.currencyGroup, option.network?.name, option.network?.id].filter(Boolean).join(" "), query)) : detailOptions;
  $: bankDetails = filteredDetails.filter(({ method }) => method.kind === "bank");
  $: cashDetails = filteredDetails.filter(({ method }) => method.kind === "cash");
  $: networkDetails = filteredDetails.filter(({ method }) => method.kind === "wallet");
</script>

{#if open}
  <div class="backdrop" on:mousedown={close} role="presentation">
    <div class:dragging class="dialog" bind:this={dialog} role="dialog" aria-modal="true" aria-label={title} tabindex="-1" on:mousedown|stopPropagation>
      <button type="button" class="sheetHandle" aria-label="Close payment method picker by dragging down" on:pointerdown={startSheetDrag} on:pointermove={moveSheetDrag} on:pointerup={endSheetDrag} on:pointercancel={endSheetDrag}><span aria-hidden="true"></span></button>
      <div class="titleBar"><div class="titleGroup">{#if stage === "detail"}<button type="button" class="stageBack" on:click={goBack} aria-label={t("Back to currencies", {}, $locale)}><svg width="17" height="17" viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="m15 18-6-6 6-6" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" /></svg></button>{/if}<div><h2 class="title">{title}</h2>{#if stage === "detail" && activeChoice}<span class="stageHint">{activeChoice.id}</span>{/if}</div></div><button type="button" class="closeButton" on:click={close} aria-label={t("Close payment method picker", {}, $locale)}><svg width="19" height="19" viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="m7 7 10 10m0-10L7 17" stroke="currentColor" stroke-width="2" stroke-linecap="round" /></svg></button></div>
      <section class="resultPanel"><div class="searchRow"><label class="searchBox"><svg width="19" height="19" viewBox="0 0 24 24" fill="none" aria-hidden="true"><circle cx="11" cy="11" r="7" stroke="currentColor" stroke-width="1.7" /><path d="m20 20-4-4" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" /></svg><input bind:this={input} bind:value={query} placeholder={t(stage === "currency" ? "Currency or digital asset" : activeChoice?.kind === "asset" ? "Search blockchains" : "Search banks and payment methods", {}, $locale)} aria-label={t(stage === "currency" ? "Currencies and digital assets" : activeChoice?.kind === "asset" ? "Blockchains" : "Search banks and payment methods", {}, $locale)} /></label></div>
        <div class="methods" role="listbox" aria-label={t(stage === "currency" ? "Currencies and digital assets" : activeChoice?.kind === "asset" ? "Blockchains" : "Payment methods", {}, $locale)}>
          {#if stage === "currency"}
            {#if visibleCurrencies.length === 0}<div class="empty"><strong>{t("No currencies or assets found", {}, $locale)}</strong><span>{t("Try a different search.", {}, $locale)}</span></div>{/if}
            {#if visibleCurrencies.some((choice) => choice.kind === "fiat")}<section class="section"><h3>{t("Fiat currencies", {}, $locale)}</h3>{#each visibleCurrencies.filter((choice) => choice.kind === "fiat") as choice (choice.id)}<PickerOptionCard name={choice.id} meta={choice.name} iconUrl={choice.iconUrl ?? null} initials={choice.initials} color={choice.color} selected={selectedCurrency === choice.id} onSelect={() => chooseCurrency(choice)} />{/each}</section>{/if}
            {#if visibleCurrencies.some((choice) => choice.kind === "asset")}<section class="section"><h3>{t("Digital assets", {}, $locale)}</h3>{#each visibleCurrencies.filter((choice) => choice.kind === "asset") as choice (choice.id)}<PickerOptionCard name={choice.id} meta={choice.name} iconUrl={choice.iconUrl ?? null} initials={choice.initials} color={choice.color} selected={selectedCurrency === choice.id} onSelect={() => chooseCurrency(choice)} />{/each}</section>{/if}
          {:else if filteredDetails.length === 0}<div class="empty"><strong>{t(activeChoice?.kind === "asset" ? "No compatible blockchains found" : "No payment methods found", {}, $locale)}</strong><span>{t("Try a different search.", {}, $locale)}</span></div>
          {:else if activeChoice?.kind === "asset"}<section class="section"><h3>{t("Available networks", {}, $locale)}</h3>{#each networkDetails as option (`${option.method.id}:${option.network?.id}`)}{@const network = option.network}<PickerOptionCard name={network?.name ?? "Network"} meta={`${option.method.currency} · ${option.method.name}`} iconUrl={network ? networkIcon(network.name) : null} initials={network?.name.slice(0, 2).toUpperCase() ?? "NW"} color="#eef2ea" selected={selected?.id === option.method.id && network?.id === selectedNetwork?.id} onSelect={() => onSelect(option.method, network)} />{/each}</section>
          {:else}{#if bankDetails.length > 0}<section class="section"><h3>{t("Banks", {}, $locale)}</h3>{#each bankDetails as option (`${option.method.id}`)}<PickerOptionCard name={option.method.name} meta={methodMeta(option.method, option.variants)} iconUrl={methodLogo(option.method, option.variants)} initials={option.method.initials} color={option.method.color} selected={selected?.id === option.method.id} onSelect={() => onSelect(option.method)} />{/each}</section>{/if}{#if cashDetails.length > 0}<section class="section"><h3>{t("Cash", {}, $locale)}</h3>{#each cashDetails as option (`${option.method.id}`)}<PickerOptionCard name={option.method.name} meta={methodMeta(option.method, option.variants)} iconUrl={methodLogo(option.method, option.variants)} initials={option.method.initials} color={option.method.color} selected={selected?.id === option.method.id} onSelect={() => onSelect(option.method)} />{/each}</section>{/if}{/if}
        </div>
      </section>
    </div>
  </div>
{/if}

<style>
  .backdrop { position: fixed; inset: 0; z-index: 1100; display: grid; place-items: center; padding: 24px; background: rgba(8, 11, 8, .52); touch-action: none; }
  .dialog { width: min(100%, 620px); max-height: min(720px, 92vh); overflow: hidden; border: 1px solid var(--color-border-strong); border-radius: 18px; background: var(--color-paper); box-shadow: var(--shadow-pop); touch-action: auto; }
  .titleBar, .titleGroup, .searchBox { display: flex; align-items: center; }
  .titleBar { min-height: 72px; justify-content: space-between; gap: 12px; padding: 14px 20px; border-bottom: 1px solid var(--color-border); }
  .titleGroup { min-width: 0; gap: 10px; }
  .title { margin: 0; font-size: 18px; font-weight: 750; letter-spacing: -.035em; }
  .stageHint { display: block; margin-top: 3px; color: var(--color-text-faint); font-family: var(--font-mono); font-size: 10px; }
  .stageBack, .closeButton { display: grid; width: 34px; height: 34px; flex: 0 0 auto; place-items: center; border-radius: 9px; color: var(--color-text-soft); }
  .stageBack:hover, .closeButton:hover { background: var(--color-panel); }
  .closeButton { width: 36px; height: 36px; }
  .sheetHandle { display: none; }
  .resultPanel { display: flex; min-height: 0; flex-direction: column; }
  .searchRow { flex: 0 0 auto; padding: 14px 18px 13px; border-bottom: 1px solid var(--color-border); }
  .searchBox { height: 48px; gap: 11px; padding: 0 16px; border: 1px solid var(--color-border); border-radius: 10px; background: var(--color-panel-soft); color: var(--color-text-faint); transition: border-color .16s ease, background .16s ease, box-shadow .16s ease, color .16s ease; }
  .searchBox:focus-within { border-color: var(--color-accent-strong); background: var(--color-paper); color: var(--color-text-soft); box-shadow: 0 0 0 4px var(--color-accent-soft); }
  .searchBox input { width: 100%; border: 0; outline: 0; background: transparent; font: inherit; font-size: 13px; font-weight: 650; }
  .searchBox input::placeholder { color: var(--color-text-faint); opacity: 1; }
  .methods { height: min(540px, calc(92vh - 160px)); min-height: 330px; overflow-y: auto; padding: 5px 14px 22px; scrollbar-width: thin; scrollbar-color: var(--color-border-strong) transparent; }
  .section + .section { margin-top: 12px; }
  .section h3 { margin: 0; padding: 14px 10px 6px; color: var(--color-text-faint); font-family: var(--font-mono); font-size: 9px; font-weight: 500; letter-spacing: .09em; text-transform: uppercase; }
  .empty { display: flex; min-height: 250px; align-items: center; justify-content: center; gap: 6px; color: var(--color-text-faint); text-align: center; flex-direction: column; }
  .empty strong { color: var(--color-text); font-size: 14px; }
  .empty span { font-size: 11px; }
  @media (max-width: 700px) { .backdrop { align-items: end; padding: 0; } .dialog { max-height: 94vh; border-right: 0; border-bottom: 0; border-left: 0; border-radius: 18px 18px 0 0; transform: translateY(var(--sheet-drag, 0)); transition: transform .24s ease; } .dialog.dragging { transition: none; } .sheetHandle { display: flex; width: 100%; height: 30px; align-items: center; justify-content: center; padding: 0; color: var(--color-text-faint); } .sheetHandle span { display: block; width: 38px; height: 5px; border-radius: 999px; background: currentColor; } .closeButton { display: none; } .methods { height: min(650px, calc(94vh - 145px)); } }
  :global(html[data-theme="dark"]) .searchBox:focus-within { background: var(--color-panel); }
</style>
