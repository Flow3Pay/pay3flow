<script lang="ts">
  import { createShortShare, type OtcPreview } from "$lib/share-links";
  import { exchangeShareImage, exchangeShareUrl, type ExchangeShareState } from "$lib/exchange-share";
  import WalletMenu from "./WalletMenu.svelte";
  import { tick, onDestroy } from "svelte";
  import { API_BASE_URL, apiUrl } from "$lib/api";
  import { locale, localeLabel, setLocale, t, type Locale } from "$lib/i18n";
  import { homeContent } from "$lib/home-content";
  import { fly } from "svelte/transition";
  const moonIcon = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAACWklEQVR4AbTVTYhNYRzH8TPCRs0kLxsWIvIywkrykvISTZRkbGiSQsqCIkuWyobkpZkiL5PQ2Eyk2BBGErOaZCMz0giRrEyNz/90z3XvzL23e6Zm+n3v/zz/5/n/f8957plzJyTj/FfTYHh4eDM60Il5Y9lLRQPN5uOyho+wH7GuQ8ytKCwr0ni7xHMcxGOsxW68QW6VGWi+TofrmIGzDQ0Nm8QpCL2Oj7wUDTRvVnwVTRqHjrsOLYoPvEJuFQ1U7sFc7ESplhoMcvwo5lZqYPcTVR5Ap0ZdYqmmG/RjTEoNVLZhKi5hpOILXzAyWe84M8jO+VOFwndyje4yW2NYvzKD2P0fx1PJoLfQLp6owmX9odRgoFIZ00H5CzjnLhaKuVRq8LdaJZMj5n7hIXIpM4jCZjuMx7Rag30m5lgzgOWu61Jm8KywuqUQRwV3cV9yI2bhLZMTmO26plIDxS+t+ow1qCrrnpichms4g34mvWjHqRLWm0uVGqRXSXJHbLWo6l2YT5j8QBzXLuMw+SbGy/CwuA0Rb4upigaKjsnE+6aLSfxnG1aX9fdwEhvQiJlW78VX9CFV0SAdJckWcTKeIpdsarGCOIUl4kWkKjOwi5+yq7FKwW/EC9CwtqzbakXWvFWfu8apygwiY/KFOAm3cENxfHktYhyB1H/JrcRNmQdoQllz42SUQSSZDOGQ66BV7MagZu/Rgw/4LteDZYjfjhVqijuXS1XRIJ3xoeAK4myDeJ1Hwy+mwvC0GE9NNI5fv3iapMpV0yBbyqQP7WjDDhzFeXRjKFtXKf4DAAD///Lx6McAAAAGSURBVAMASQPNMX2ya7kAAAAASUVORK5CYII=";
  const sunIcon = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAACD0lEQVR4AbSTPS8FQRSG92o0+Ae0EkKDhk58NJQUoiASlUoQKolCoySESCQaCdEp0BGREBQK4W+goJDreSdnstmdXXe3cHOenDNn3nfO7t7duuiff6UGVKvVWVHmmkoNKHOw15YaUKlU9oQ3F8nBAB7BBSxlmem3ipy9ZfbO03vBAARX8AkuMOm535DfabwK1aDeLGsf8lz7hc/BAB7BOmxLwCEP5F1ogn2YMFSrt2uaCM8WrLOfiGCA38V4TN0Fl9CLeR6OjHn1QHtdpmUZRuYADCNIx0BXNcyhun2WcagHw3S2YMw8lMnIHIBkBZ5gAWqFNNLKE2jzBnSivOcKv8l/hmnuEclDSoYbwO3pTVnUFnUbuQHuoGhI22DeiLwI7g1zA1Kn/Ni65tWbTslrvVc9hxvAbeoL3VCH+o38AT1QNKT9MK9e2Q3qPZndABUpnlkPcJt6VJT5YZoBFPKQkpE34BBZB5xCrZBGWnkCbeYAu71H1ENc4SbUUydCPdikOQSP5qFMRuYASTB0k09gDm45bAemjB31QHsnpmUZRjCAA1ZgWlKM4+RR+IJJODBUqzdqGr2aM/iW2U9EMIDdfmgGFxxwBn3QSKNdqAb1zlj7aKGQlxRHMADjIKzFkrii/yLiTlzRXwX9H3GTKhhALzd4BPri3ReaK0ptlBqQ8hZalhrAI9AX777QQqdHUfQLAAD//91ClIsAAAAGSURBVAMAR3zSMQ+aPXkAAAAASUVORK5CYII=";
  const menuIcon = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAAAdElEQVR4AexTwQ3AIAgsXaSzdCpncKrO0knoSYq/xpKoD3OEUx4I8S63b4ODC5oEL0SRqibgBjwvFBGg3bLMSM7dPIpEJAMH4HmiiADtlmVGnv8D39j7rhpAHopsCuOgyPrlbn2DTo5ZsRot9ux/Nxc0uXoAAAD//8ndUOoAAAAGSURBVAMAa9HwMZNspewAAAAASUVORK5CYII=";


  export let shareState: ExchangeShareState | null = null;
  export let activePage: "swap" | "otc" | "about" = "swap";
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
  let shareOpen = false;
  let shareLoading = false;
  async function openShare() {
    if (shareLoading) return;
    sharingOtc = activePage === "otc" && Boolean(shareUrl);
    if (!shareState && shareUrl && !sharingOtc) {
      try { await navigator.clipboard.writeText(new URL(shareUrl, location.origin).toString()); shareMessage = t("Link copied", {}, activeLocale); }
      catch { shareMessage = t("Could not copy link", {}, activeLocale); }
      clearTimeout(shareMessageTimer);
      shareMessageTimer = setTimeout(() => shareMessage = "", 3000);
      return;
    }
    if (!shareState && !sharingOtc) return;
    shareLoading = true;
    try {
      if (menuOpen) await closeMenu();
      if (sharingOtc) {
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
  $: activeLanguage = languages.find(language => language.locale === activeLocale)!;
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
    <nav class="productNav" aria-label={t("Exchange mode", {}, activeLocale)}><a href={swapHref} class:active={activePage === "swap"} aria-current={activePage === "swap" ? "page" : undefined}>SWAP</a><a href={otcHref} on:click={onOtcNavigate} class:active={activePage === "otc"} aria-current={activePage === "otc" ? "page" : undefined}>OTC</a></nav>
    <div class="utilityActions">
      <WalletMenu />
      <div class="languageWrap" bind:this={languageWrap} on:focusout={languageFocus}>
        <button class="languageToggle" bind:this={languageButton} type="button" on:click={() => languageOpen ? closeLanguages() : openLanguages()} aria-label={t("Switch language", {}, activeLocale)} aria-haspopup="listbox" aria-controls="header-language-menu" aria-expanded={languageOpen}><img class="languageFlag" src={`/icons/flags/${activeLanguage.flag}.svg`} alt="" width="22" height="22" /><span class="languageCode">{localeLabel(activeLocale)}</span><svg class="languageChevron" class:open={languageOpen} width="12" height="12" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="m4 6 4 4 4-4" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" /></svg></button>
        {#if languageOpen}<div class="languageMenu" id="header-language-menu" role="listbox" aria-label={t("Switch language", {}, activeLocale)}>{#each languages as language}<button type="button" role="option" tabindex="-1" aria-selected={activeLocale === language.locale} on:click={() => selectLanguage(language.locale)}><img class="languageFlag" src={`/icons/flags/${language.flag}.svg`} alt="" width="22" height="22" /><span lang={language.locale}>{language.name}</span>{#if activeLocale === language.locale}<span class="languageCheck" aria-hidden="true">✓</span>{/if}</button>{/each}</div>{/if}
      </div>
      <button class="themeToggle" type="button" on:click={toggleTheme} aria-label={t("Switch theme", {}, activeLocale)}><img class="moonIcon" src={moonIcon} alt="" width="20" height="20" /><img class="sunIcon" src={sunIcon} alt="" width="20" height="20" /></button>
      {#if shareState || shareUrl}<button type="button" class="shareButton" aria-label={t(activePage === "otc" ? "Share OTC" : "Share exchange", {}, activeLocale)} title={t(activePage === "otc" ? "Share OTC" : "Share exchange", {}, activeLocale)} aria-haspopup={shareState || activePage === "otc" ? "dialog" : undefined} aria-expanded={shareState || activePage === "otc" ? shareOpen : undefined} aria-busy={shareLoading} on:click={openShare}><img src="/icons/ui/share.png" alt="" width="20" height="20" /></button>{/if}
      {#if shareMessage}<span class="shareMessage" role="status">{shareMessage}</span>{/if}
    </div>
  </div>
</header>
{#if shareOpen && ShareDialog}<svelte:component this={ShareDialog} state={sharedState} url={shareLink} imageUrl={shareImage} shareTitle={sharingOtc ? "Share OTC" : "Share exchange"} shareDescription={sharingOtc ? "Send this link to open the same OTC settings." : "Send this link to open the same exchange settings."} linkLabel={sharingOtc ? "OTC link" : "Exchange link"} previewAlt={sharingOtc ? "Pay3Flow OTC · Bridge · Chart · Orderbook" : ""} onClose={() => shareOpen = false} />{/if}

<style>
.header { position: relative; z-index: 50; padding: 16px var(--page-gutter) 8px; }
.inner { display: flex; width: min(var(--layout-width), 100%); min-height: 58px; align-items: center; gap: 14px; margin: 0 auto; padding: 6px 8px; border: 1px solid var(--color-border); border-radius: 13px; background: var(--color-paper); }
.brand { display: flex; min-width: 0; align-items: center; gap: 9px; }
.logo { display: block; flex: 0 0 auto; }
.wordmark { font-size: 17px; font-weight: 800; letter-spacing: -.045em; }
.productNav { display: flex; gap: 4px; margin-left: 14px; }
.productNav a { display: flex; align-items: center; justify-content: center; gap: 7px; padding: 10px 17px; border-radius: 8px; font-size: 12px; font-weight: 800; letter-spacing: .035em; color: var(--color-text-soft); transition: background .16s; }
.productNav a:hover { background: var(--color-panel); }
.productNav a.active { background: var(--color-accent-soft); color: var(--color-text); }
.utilityActions { position: relative; display: flex; align-items: center; gap: 8px; margin-left: auto; }
.shareButton, .aboutLink, .githubLink, .telegramLink, .themeToggle, .languageToggle, .apiDocsLink, .menuToggle { display: grid; width: 36px; height: 36px; flex: 0 0 auto; place-items: center; border: 1px solid var(--color-border); border-radius: 9px; background: var(--color-panel); transition: background .16s ease; }
.shareButton:hover, .aboutLink:hover, .githubLink:hover, .telegramLink:hover, .themeToggle:hover, .languageToggle:hover, .apiDocsLink:hover, .menuToggle:hover { background: var(--color-accent-soft); }
.shareButton img, .githubLink svg, .telegramLink img { display: block; width: 20px; height: 20px; }
.languageToggle, .apiDocsLink { color: var(--color-text-soft); font-size: 12px; font-weight: 800; }
.languageWrap { position: relative; flex: 0 0 auto; }
.languageToggle { display: flex; width: auto; min-width: 72px; gap: 6px; align-items: center; justify-content: center; padding: 0 8px; }
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
@media (max-width: 640px) {
  .header { padding-top: 12px; }
  .inner { gap: 8px; padding: 6px; flex-wrap: wrap; }
  .brand { gap: 6px; min-width: 44px; min-height: 44px; justify-content: center; }
  .logo { width: 28px; height: 28px; }
  .wordmark { display: none; }
  .utilityActions { gap: 6px; }
  .themeToggle, .languageToggle, .shareButton, .menuToggle { width: 44px; height: 44px; }
  .languageToggle { min-width: 44px; padding: 0; gap: 3px; }
  .languageCode { display: none; }
  .productNav { order: 3; width: 100%; margin: 0; border-top: 1px solid var(--color-border); padding-top: 6px; }
  .productNav a { flex: 1; padding: 8px; }
}
@media (prefers-reduced-motion: reduce) { .languageChevron, .languageMenu button { transition: none; } .languageMenu { animation: none; } }
@media (max-width: 360px) { .logo { width: 24px; height: 24px; } .inner { gap: 3px; } .utilityActions { gap: 3px; } }
</style>
