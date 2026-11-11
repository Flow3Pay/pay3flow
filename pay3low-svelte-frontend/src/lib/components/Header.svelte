<script lang="ts">
  import { createShortShare, type OtcPreview } from "$lib/share-links";
  import { exchangeShareImage, exchangeShareUrl, type ExchangeShareState } from "$lib/exchange-share";
  import WalletMenu from "./WalletMenu.svelte";
  import { tick, onDestroy } from "svelte";
  import { API_BASE_URL, apiUrl } from "$lib/api";
  import { locale, setLocale, t, type Locale } from "$lib/i18n";
  import { homeContent } from "$lib/home-content";
  import ScrambleLink from "./ScrambleLink.svelte";
  import { fly } from "svelte/transition";
  const moonIcon = "/icons/ui/moon.svg";
  const sunIcon = "/icons/ui/sun.svg";
  const menuIcon = "/icons/ui/menu.svg";


  export let shareState: ExchangeShareState | null = null;
  export let activePage: "swap" | "otc" | "futures" | "about" | "providers" | "trends" = "swap";
  export let swapHref = "/#/swap";
  export let otcHref = "/#/otc";
  export let onOtcNavigate: () => void = () => {};
  export let otcPreview: OtcPreview | null = null;
  export let shareUrl: string | null = null;
  let shareMessage = "";
  let shareMessageTimer: ReturnType<typeof setTimeout> | undefined;
  let ShareDialog: typeof import("./ExchangeShareDialog.svelte").default | null = null;
  let sharedState: ExchangeShareState | null = null;
  let shareLink = "";
  let shareImage: string | null = null;
  let sharingOtc = false;
  let sharingTrends = false;
  $: shareTitle = activePage === "trends" ? "Share trends" : activePage === "otc" ? "Share OTC" : "Share exchange";
  $: shareHasDialog = Boolean(shareState) || activePage === "otc" || activePage === "trends";
  let shareOpen = false;
  let shareLoading = false;
  async function openShare() {
    if (shareLoading) return;
    sharingOtc = activePage === "otc" && Boolean(shareUrl);
    sharingTrends = activePage === "trends" && Boolean(shareUrl);
    if (!shareState && shareUrl && !sharingOtc && !sharingTrends) {
      try { await navigator.clipboard.writeText(new URL(shareUrl, location.origin).toString()); shareMessage = t("Link copied", {}, activeLocale); }
      catch { shareMessage = t("Could not copy link", {}, activeLocale); }
      clearTimeout(shareMessageTimer);
      shareMessageTimer = setTimeout(() => shareMessage = "", 3000);
      return;
    }
    if (!shareState && !sharingOtc && !sharingTrends) return;
    shareLoading = true;
    try {
      if (menuOpen) await closeMenu();
      if (sharingTrends) {
        sharedState = null;
        const target = new URL(shareUrl!, location.origin);
        target.searchParams.set("lang", activeLocale);
        shareLink = target.toString();
        shareImage = `${target.origin}/trends/preview.png?lang=${activeLocale}&v=1`;
      } else if (sharingOtc) {
        sharedState = null;
        const target = `${shareUrl}&lang=${activeLocale}`;
        shareLink = await createShortShare(target, otcPreview);
        shareImage = `${shareLink}/preview.png?v=3`;
      } else if (shareState) {
        sharedState = { ...shareState, sources: [...shareState.sources], methods: [...shareState.methods], assets: [...shareState.assets] };
        const fullUrl = exchangeShareUrl(location.origin, sharedState, activeLocale);
        const parsed = new URL(fullUrl);
        shareImage = exchangeShareImage(location.origin, sharedState.source, sharedState.target, parsed.searchParams);
        shareLink = await createShortShare(`${parsed.pathname}${parsed.search}`);
      }
      ShareDialog ??= (await import("./ExchangeShareDialog.svelte")).default;
      shareOpen = true;
    } catch {
      shareMessage = t("Could not create share link. Try again.", {}, activeLocale);
      clearTimeout(shareMessageTimer);
      shareMessageTimer = setTimeout(() => shareMessage = "", 5000);
    } finally { shareLoading = false; }
  }
  const apiDocsHref = API_BASE_URL ? apiUrl("/scalar").toString() : "/scalar";
  let menuOpen = false;
  let menuDuration = 180;
  let menuWrap: HTMLDivElement;
  let menuPanel: HTMLDivElement;
  let menuToggle: HTMLButtonElement;
  $: activeLocale = $locale;
  const languages: { locale: Locale; name: string; flag: string }[] = [
    { locale: "en", name: "English", flag: "us" },
    { locale: "ru", name: "Русский", flag: "ru" },
    { locale: "hy", name: "Հայերեն", flag: "am" },
  ];
  let languageOpen = false;
  let languageWrap: HTMLDivElement;
  let languageButton: HTMLButtonElement;
  async function openLanguages() {
    languageOpen = true;
    await tick();
    languageWrap.querySelector<HTMLButtonElement>('[aria-selected="true"]')?.focus();
  }
  function closeLanguages(restoreFocus = false) {
    languageOpen = false;
    if (restoreFocus) languageButton?.focus();
  }
  function selectLanguage(language: Locale) { setLocale(language); closeLanguages(true); }
  function outsideLanguage(event: PointerEvent) {
    if (languageOpen && !languageWrap.contains(event.target as Node)) closeLanguages();
  }
  function languageFocus(event: FocusEvent) {
    if (languageOpen && !languageWrap.contains(event.relatedTarget as Node | null)) closeLanguages();
  }
  function languageKeys(event: KeyboardEvent) {
    if (!languageOpen) {
      if (event.target === languageButton && ["ArrowDown", "ArrowUp"].includes(event.key)) { event.preventDefault(); void openLanguages(); }
      return;
    }
    if (event.key === "Escape") { event.preventDefault(); event.stopImmediatePropagation(); closeLanguages(true); }
    else if (["ArrowUp", "ArrowDown", "Home", "End"].includes(event.key)) {
      event.preventDefault();
      const options = [...languageWrap.querySelectorAll<HTMLButtonElement>('[role="option"]')];
      const index = options.indexOf(document.activeElement as HTMLButtonElement);
      const next = event.key === "Home" ? 0 : event.key === "End" ? options.length - 1 : (index + (event.key === "ArrowDown" ? 1 : -1) + options.length) % options.length;
      options[next]?.focus();
    }
  }

  async function openMenu(last = false) {
    closeLanguages();
    menuDuration = window.matchMedia("(prefers-reduced-motion: reduce)").matches ? 0 : 180;
    menuOpen = true;
    await tick();
    const items = menuPanel?.querySelectorAll<HTMLAnchorElement>('[role="menuitem"]');
    (last ? items?.[items.length - 1] : items?.[0])?.focus();
  }
  function closeMenu(restoreFocus = false) {
    menuOpen = false;
    if (restoreFocus) menuToggle?.focus();
  }
  function outsideMenu(event: PointerEvent) {
    if (menuOpen && !menuWrap.contains(event.target as Node)) closeMenu();
  }
  function menuFocus(event: FocusEvent) {
    if (menuOpen && !menuWrap.contains(event.relatedTarget as Node | null)) closeMenu();
  }
  function onKeyDown(event: KeyboardEvent) {
    if (!menuOpen) {
      if (event.target === menuToggle && ["ArrowDown", "ArrowUp"].includes(event.key)) {
        event.preventDefault();
        void openMenu(event.key === "ArrowUp");
      }
      return;
    }
    if (event.key === "Escape") { event.preventDefault(); closeMenu(true); }
    else if (["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
      event.preventDefault();
      const items = [...menuPanel.querySelectorAll<HTMLAnchorElement>('[role="menuitem"]')];
      const index = items.indexOf(document.activeElement as HTMLAnchorElement);
      const next = event.key === "Home" ? 0 : event.key === "End" ? items.length - 1 : (index + (event.key === "ArrowDown" ? 1 : -1) + items.length) % items.length;
      items[next]?.focus();
    }
  }
  onDestroy(() => clearTimeout(shareMessageTimer));
  function toggleTheme() {
    const root = document.documentElement;
    const theme = root.dataset.theme === "dark" ? "light" : "dark";
    root.dataset.theme = theme;
    root.dataset.themeManual = "true";
    try { localStorage.setItem("pay3flow-theme", theme); } catch {}
  }
</script>

<svelte:window on:keydown={(event) => { languageKeys(event); if (!event.defaultPrevented) onKeyDown(event); }} on:pointerdown={(event) => { outsideLanguage(event); outsideMenu(event); }} />
<header class="header">
  <div class="inner">
    <div class="menuWrap" bind:this={menuWrap} on:focusout={menuFocus}>
      <button class="menuToggle" bind:this={menuToggle} type="button" aria-haspopup="menu" aria-controls="header-menu" aria-expanded={menuOpen} aria-label={t("Open menu", {}, activeLocale)} on:click={() => menuOpen ? closeMenu() : openMenu()}><img src={menuIcon} alt="" width="24" height="24" /></button>
      {#if menuOpen}
        <div id="header-menu" class="actionsPanel" bind:this={menuPanel} role="menu" aria-label={t("Menu", {}, activeLocale)} in:fly={{ y: -6, duration: menuDuration }} out:fly={{ y: -4, duration: menuDuration * 2 / 3 }}>
        <div class="actions">

          <a role="menuitem" tabindex="-1" class="aboutLink" href="/about" aria-current={activePage === "about" ? "page" : undefined} on:click={() => closeMenu()}><img src="/icons/assets/pay3flow-mark.svg" alt="" width="20" height="20" /><span>{homeContent[activeLocale].aboutTitle}</span></a>
          <a role="menuitem" tabindex="-1" class="apiDocsLink" href={apiDocsHref} target="_blank" rel="noreferrer noopener" aria-label={t("Open API documentation", {}, activeLocale)} on:click={() => closeMenu()}>API DOCS</a>
          <a role="menuitem" tabindex="-1" class="telegramLink" href="https://t.me/pay3flow" target="_blank" rel="noreferrer noopener" aria-label={t("Open Pay3Flow Telegram channel", {}, activeLocale)} on:click={() => closeMenu()}><img src="/icons/assets/telegram-messenger.png" alt="" width="20" height="20" /><span class="mobileActionLabel">Telegram</span></a>
          <a role="menuitem" tabindex="-1" class="githubLink" href="https://github.com/Flow3Pay/pay3flow" target="_blank" rel="noreferrer noopener" aria-label={t("Open Pay3Flow on GitHub", {}, activeLocale)} on:click={() => closeMenu()}><svg viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M12 .7a11.3 11.3 0 0 0-3.58 22.02c.57.1.78-.25.78-.55v-2.16c-3.18.7-3.85-1.34-3.85-1.34-.52-1.32-1.27-1.67-1.27-1.67-1.04-.71.08-.7.08-.7 1.15.08 1.76 1.18 1.76 1.18 1.02 1.75 2.68 1.24 3.34.95.1-.74.4-1.24.73-1.53-2.54-.29-5.2-1.27-5.2-5.65 0-1.25.45-2.26 1.18-3.06-.12-.29-.51-1.45.11-3.02 0 0 .96-.31 3.12 1.17a10.8 10.8 0 0 1 5.68 0c2.16-1.48 3.12-1.17 3.12-1.17.62 1.57.23 2.73.11 3.02.73.8 1.18 1.81 1.18 3.06 0 4.39-2.67 5.35-5.21 5.64.41.36.78 1.08.78 2.18v3.23c0 .3.2.65.79.54A11.3 11.3 0 0 0 12 .7Z" /></svg><span class="mobileActionLabel">GitHub</span></a>
        </div>
        </div>
      {/if}
    </div>
    <a class="brand" href={swapHref} aria-label="Pay3Flow"><img class="logo" src="/icons/assets/pay3flow-mark.svg" alt="" width="34" height="34" /><span class="wordmark">Pay3Flow</span></a>
    <nav class="productNav" aria-label={t("Exchange mode", {}, activeLocale)}><ScrambleLink href={swapHref} label="SWAP" active={activePage === "swap"} current={activePage === "swap" ? "page" : undefined} />
      <!-- Futures navigation paused by product decision; retain draft link for later.
      <ScrambleLink href="/futures" label={activeLocale === "ru" ? "ФЬЮЧЕРСЫ" : activeLocale === "hy" ? "ՖՅՈՒՉԵՐՍՆԵՐ" : "FUTURES"} active={activePage === "futures"} current={activePage === "futures" ? "page" : undefined} />
      -->
      <ScrambleLink href={otcHref} label="OTC" active={activePage === "otc"} current={activePage === "otc" ? "page" : undefined} onNavigate={onOtcNavigate} /><ScrambleLink href="/trends" label={t("TRENDS", {}, activeLocale)} active={activePage === "trends"} current={activePage === "trends" ? "page" : undefined} /><ScrambleLink href="/providers" label={t("Platforms", {}, activeLocale).toLocaleUpperCase(activeLocale)} active={activePage === "providers"} current={activePage === "providers" ? "page" : undefined} /></nav>
    <div class="utilityActions">
      <WalletMenu />
      <div class="languageWrap" bind:this={languageWrap} on:focusout={languageFocus}>
        <button class="languageToggle" bind:this={languageButton} type="button" on:click={() => languageOpen ? closeLanguages() : openLanguages()} aria-label={t("Switch language", {}, activeLocale)} aria-haspopup="listbox" aria-controls="header-language-menu" aria-expanded={languageOpen}><svg class="languageGlobe" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M12 22C6.477 22 2 17.523 2 12C2 6.477 6.477 2 12 2C17.523 2 22 6.477 22 12C22 17.523 17.523 22 12 22ZM9.71 19.667C8.72341 17.5743 8.15187 15.3102 8.027 13H4.062C4.25659 14.5389 4.89392 15.9882 5.89657 17.1717C6.89922 18.3552 8.22401 19.2221 9.71 19.667V19.667ZM10.03 13C10.181 15.439 10.878 17.73 12 19.752C13.1523 17.6766 13.8254 15.3695 13.97 13H10.03V13ZM19.938 13H15.973C15.8481 15.3102 15.2766 17.5743 14.29 19.667C15.776 19.2221 17.1008 18.3552 18.1034 17.1717C19.1061 15.9882 19.7434 14.5389 19.938 13V13ZM4.062 11H8.027C8.15187 8.68979 8.72341 6.42569 9.71 4.333C8.22401 4.77788 6.89922 5.64475 5.89657 6.8283C4.89392 8.01184 4.25659 9.4611 4.062 11V11ZM10.031 11H13.969C13.8248 8.6306 13.152 6.32353 12 4.248C10.8477 6.32345 10.1746 8.63052 10.03 11H10.031ZM14.29 4.333C15.2766 6.42569 15.8481 8.68979 15.973 11H19.938C19.7434 9.4611 19.1061 8.01184 18.1034 6.8283C17.1008 5.64475 15.776 4.77788 14.29 4.333V4.333Z" /></svg><svg class="languageChevron" class:open={languageOpen} width="12" height="12" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="m4 6 4 4 4-4" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" /></svg></button>
        {#if languageOpen}<div class="languageMenu" id="header-language-menu" role="listbox" aria-label={t("Switch language", {}, activeLocale)}>{#each languages as language}<button type="button" role="option" tabindex="-1" aria-selected={activeLocale === language.locale} on:click={() => selectLanguage(language.locale)}><img class="languageFlag" src={`/icons/flags/${language.flag}.svg`} alt="" width="22" height="22" /><span lang={language.locale}>{language.name}</span>{#if activeLocale === language.locale}<span class="languageCheck" aria-hidden="true">✓</span>{/if}</button>{/each}</div>{/if}
      </div>
      <button class="themeToggle" type="button" on:click={toggleTheme} aria-label={t("Switch theme", {}, activeLocale)}><img class="moonIcon" src={moonIcon} alt="" width="20" height="20" /><img class="sunIcon" src={sunIcon} alt="" width="20" height="20" /></button>
      {#if shareState || shareUrl}<button type="button" class="shareButton" aria-label={t(shareTitle, {}, activeLocale)} title={t(shareTitle, {}, activeLocale)} aria-haspopup={shareHasDialog ? "dialog" : undefined} aria-expanded={shareHasDialog ? shareOpen : undefined} aria-busy={shareLoading} on:click={openShare}><img src="/icons/ui/share.svg" alt="" width="20" height="20" /></button>{/if}
      {#if shareMessage}<span class="shareMessage" role="status">{shareMessage}</span>{/if}
    </div>
  </div>
</header>
{#if shareOpen && ShareDialog}<svelte:component this={ShareDialog} state={sharedState} url={shareLink} imageUrl={shareImage} shareTitle={sharingTrends ? "Share trends" : sharingOtc ? "Share OTC" : "Share exchange"} shareDescription={sharingTrends ? "Send this link to open the exchange trend wall." : sharingOtc ? "Send this link to open the same OTC settings." : "Send this link to open the same exchange settings."} linkLabel={sharingTrends ? "Trends link" : sharingOtc ? "OTC link" : "Exchange link"} previewAlt={sharingTrends ? t("Exchange trend wall", {}, activeLocale) : sharingOtc ? "Pay3Flow OTC · Bridge · Chart · Orderbook" : ""} onClose={() => shareOpen = false} />{/if}

<style>
.header { position: relative; z-index: 50; padding: 16px var(--page-gutter) 8px; }
.inner { display: flex; width: min(var(--layout-width), 100%); min-height: 58px; align-items: center; gap: 14px; margin: 0 auto; padding: 6px 8px; border: 1px solid var(--color-border); border-radius: 13px; background: var(--color-paper); }
.brand { display: flex; min-width: 0; align-items: center; gap: 9px; }
.logo { display: block; flex: 0 0 auto; }
.wordmark { font-size: 17px; font-weight: 800; letter-spacing: -.045em; }
.productNav { display: flex; gap: 4px; margin-left: 14px; }
.productNav :global(a) { display: flex; align-items: center; justify-content: center; gap: 7px; padding: 10px 17px; border-radius: 8px; font-size: 12px; font-weight: 800; letter-spacing: .035em; color: var(--color-text-soft); transition: background .16s, color .16s; }
.productNav :global(a:hover), .productNav :global(a:focus-visible) { color: var(--color-text); }
.productNav :global(a:hover) { background: var(--color-panel); }
.productNav :global(a.active) { background: var(--color-accent-soft); color: var(--color-text); }
.utilityActions { position: relative; display: flex; align-items: center; gap: 8px; margin-left: auto; }
.shareButton, .aboutLink, .githubLink, .telegramLink, .themeToggle, .languageToggle, .apiDocsLink, .menuToggle { display: grid; width: 36px; height: 36px; flex: 0 0 auto; place-items: center; border: 1px solid var(--color-border); border-radius: 9px; background: var(--color-panel); transition: background .16s ease; }
.shareButton:hover, .aboutLink:hover, .githubLink:hover, .telegramLink:hover, .themeToggle:hover, .languageToggle:hover, .apiDocsLink:hover, .menuToggle:hover { background: var(--color-accent-soft); }
.shareButton img, .githubLink svg, .telegramLink img { display: block; width: 20px; height: 20px; }
.languageToggle, .apiDocsLink { color: var(--color-text-soft); font-size: 12px; font-weight: 800; }
.languageWrap { position: relative; flex: 0 0 auto; }
.languageToggle { display: flex; width: auto; min-width: 52px; gap: 6px; align-items: center; justify-content: center; padding: 0 8px; }
.languageGlobe { display: block; width: 20px; height: 20px; flex: 0 0 auto; }
.languageFlag { display: block; flex: 0 0 auto; border-radius: 4px; }
.languageChevron { transition: transform .18s ease; }
.languageChevron.open { transform: rotate(180deg); }
.languageMenu { position: absolute; top: calc(100% + 9px); right: 0; z-index: 80; width: 190px; padding: 8px; border: 1px solid var(--color-border); border-radius: 20px; background: var(--color-paper); animation: languageIn .18s ease-out; }
.languageMenu button { display: flex; width: 100%; min-height: 44px; align-items: center; gap: 10px; padding: 8px 10px; border-radius: 7px; text-align: left; font-size: 13px; transition: background .15s ease; }
.languageMenu button:hover, .languageMenu button:focus-visible, .languageMenu button[aria-selected="true"] { background: var(--color-accent-soft); }
.languageCheck { margin-left: auto; color: var(--color-accent-text); }
@keyframes languageIn { from { opacity: 0; transform: translateY(-5px); } }
.shareButton img, .menuToggle img { filter: brightness(0); }
:global(html[data-theme="dark"]) .shareButton img, :global(html[data-theme="dark"]) .menuToggle img { filter: brightness(0) invert(1); }
.themeToggle { position: relative; }
.themeToggle img { position: absolute; }
.moonIcon { filter: brightness(0); }
.sunIcon { display: none; }
:global(html[data-theme="dark"]) .moonIcon { display: none; }
:global(html[data-theme="dark"]) .sunIcon { display: block; filter: brightness(0) invert(1); }
.shareMessage { position: absolute; top: calc(100% + 14px); right: 0; padding: 10px 14px; border: 1px solid var(--color-border); border-radius: 10px; background: var(--color-paper); font-size: 12px; white-space: nowrap; }
.menuWrap { position: relative; flex: 0 0 auto; }
.actionsPanel { position: absolute; top: calc(100% + 9px); left: 0; z-index: 80; width: min(280px, calc(100vw - 48px)); max-height: calc(100dvh - 120px); overflow-y: auto; padding: 8px; border: 1px solid var(--color-border); border-radius: 13px; background: var(--color-paper); box-shadow: 0 12px 32px rgba(0, 0, 0, .12); }
.actions { display: grid; gap: 4px; }
.aboutLink, .githubLink, .telegramLink, .apiDocsLink { display: flex; width: 100%; min-height: 44px; height: auto; align-items: center; gap: 10px; padding: 10px 12px; border: 0; border-radius: 7px; background: transparent; font-size: 13px; text-align: left; }
.actions a:hover, .actions a:focus-visible { background: var(--color-accent-soft); }
.aboutLink img { flex: 0 0 auto; }
@media (min-width: 641px) and (max-width: 1000px) {
  .inner { flex-wrap: wrap; }
  .productNav { order: 3; width: 100%; margin: 0; padding-top: 6px; border-top: 1px solid var(--color-border); }
  .productNav :global(a) { flex: 1; padding: 8px; }
}
@media (max-width: 640px) {
  .header { padding-top: 12px; }
  .inner { gap: 8px; padding: 6px; flex-wrap: wrap; }
  .brand { gap: 6px; min-width: 44px; min-height: 44px; justify-content: center; }
  .logo { width: 28px; height: 28px; }
  .wordmark { display: none; }
  .utilityActions { gap: 6px; }
  .themeToggle, .languageToggle, .shareButton, .menuToggle { width: 44px; height: 44px; }
  .languageToggle { min-width: 44px; padding: 0; gap: 3px; }
  .productNav { order: 3; width: 100%; margin: 0; border-top: 1px solid var(--color-border); padding-top: 6px; }
  .productNav :global(a) { flex: 1; padding: 8px; }
}
@media (prefers-reduced-motion: reduce) { .languageChevron, .languageMenu button { transition: none; } .languageMenu { animation: none; } }
@media (max-width: 360px) { .logo { width: 24px; height: 24px; } .inner { gap: 3px; } .utilityActions { gap: 3px; } }
</style>
