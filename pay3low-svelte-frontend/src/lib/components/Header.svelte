<script lang="ts">
  import { exchangeShareUrl, type ExchangeShareState } from "$lib/exchange-share";
  import WalletMenu from "./WalletMenu.svelte";
  import { tick } from "svelte";
  import { API_BASE_URL, apiUrl } from "$lib/api";
  import { cycleLocale, locale, localeLabel, setLocale, t } from "$lib/i18n";
  import { lockPageScroll } from "$lib/page-scroll-lock";
  const moonIcon = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAACWklEQVR4AbTVTYhNYRzH8TPCRs0kLxsWIvIywkrykvISTZRkbGiSQsqCIkuWyobkpZkiL5PQ2Eyk2BBGErOaZCMz0giRrEyNz/90z3XvzL23e6Zm+n3v/zz/5/n/f8957plzJyTj/FfTYHh4eDM60Il5Y9lLRQPN5uOyho+wH7GuQ8ytKCwr0ni7xHMcxGOsxW68QW6VGWi+TofrmIGzDQ0Nm8QpCL2Oj7wUDTRvVnwVTRqHjrsOLYoPvEJuFQ1U7sFc7ESplhoMcvwo5lZqYPcTVR5Ap0ZdYqmmG/RjTEoNVLZhKi5hpOILXzAyWe84M8jO+VOFwndyje4yW2NYvzKD2P0fx1PJoLfQLp6owmX9odRgoFIZ00H5CzjnLhaKuVRq8LdaJZMj5n7hIXIpM4jCZjuMx7Rag30m5lgzgOWu61Jm8KywuqUQRwV3cV9yI2bhLZMTmO26plIDxS+t+ow1qCrrnpichms4g34mvWjHqRLWm0uVGqRXSXJHbLWo6l2YT5j8QBzXLuMw+SbGy/CwuA0Rb4upigaKjsnE+6aLSfxnG1aX9fdwEhvQiJlW78VX9CFV0SAdJckWcTKeIpdsarGCOIUl4kWkKjOwi5+yq7FKwW/EC9CwtqzbakXWvFWfu8apygwiY/KFOAm3cENxfHktYhyB1H/JrcRNmQdoQllz42SUQSSZDOGQ66BV7MagZu/Rgw/4LteDZYjfjhVqijuXS1XRIJ3xoeAK4myDeJ1Hwy+mwvC0GE9NNI5fv3iapMpV0yBbyqQP7WjDDhzFeXRjKFtXKf4DAAD///Lx6McAAAAGSURBVAMASQPNMX2ya7kAAAAASUVORK5CYII=";
  const sunIcon = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAACD0lEQVR4AbSTPS8FQRSG92o0+Ae0EkKDhk58NJQUoiASlUoQKolCoySESCQaCdEp0BGREBQK4W+goJDreSdnstmdXXe3cHOenDNn3nfO7t7duuiff6UGVKvVWVHmmkoNKHOw15YaUKlU9oQ3F8nBAB7BBSxlmem3ipy9ZfbO03vBAARX8AkuMOm535DfabwK1aDeLGsf8lz7hc/BAB7BOmxLwCEP5F1ogn2YMFSrt2uaCM8WrLOfiGCA38V4TN0Fl9CLeR6OjHn1QHtdpmUZRuYADCNIx0BXNcyhun2WcagHw3S2YMw8lMnIHIBkBZ5gAWqFNNLKE2jzBnSivOcKv8l/hmnuEclDSoYbwO3pTVnUFnUbuQHuoGhI22DeiLwI7g1zA1Kn/Ni65tWbTslrvVc9hxvAbeoL3VCH+o38AT1QNKT9MK9e2Q3qPZndABUpnlkPcJt6VJT5YZoBFPKQkpE34BBZB5xCrZBGWnkCbeYAu71H1ENc4SbUUydCPdikOQSP5qFMRuYASTB0k09gDm45bAemjB31QHsnpmUZRjCAA1ZgWlKM4+RR+IJJODBUqzdqGr2aM/iW2U9EMIDdfmgGFxxwBn3QSKNdqAb1zlj7aKGQlxRHMADjIKzFkrii/yLiTlzRXwX9H3GTKhhALzd4BPri3ReaK0ptlBqQ8hZalhrAI9AX777QQqdHUfQLAAD//91ClIsAAAAGSURBVAMAR3zSMQ+aPXkAAAAASUVORK5CYII=";
  const menuIcon = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAAAdElEQVR4AexTwQ3AIAgsXaSzdCpncKrO0knoSYq/xpKoD3OEUx4I8S63b4ODC5oEL0SRqibgBjwvFBGg3bLMSM7dPIpEJAMH4HmiiADtlmVGnv8D39j7rhpAHopsCuOgyPrlbn2DTo5ZsRot9ux/Nxc0uXoAAAD//8ndUOoAAAAGSURBVAMAa9HwMZNspewAAAAASUVORK5CYII=";


  export let shareState: ExchangeShareState | null = null;
  let ShareDialog: typeof import("./ExchangeShareDialog.svelte").default | null = null;
  let sharedState: ExchangeShareState | null = null;
  let shareLink = "";
  let shareOpen = false;
  let shareLoading = false;
  async function openShare() {
    if (!shareState || shareLoading) return;
    sharedState = { ...shareState, sources: [...shareState.sources], methods: [...shareState.methods], assets: [...shareState.assets] };
    shareLink = exchangeShareUrl(location.origin, sharedState, activeLocale);
    shareLoading = true;
    try {
      if (menuOpen) await closeMenu();
      ShareDialog ??= (await import("./ExchangeShareDialog.svelte")).default;
      shareOpen = true;
    } finally { shareLoading = false; }
  }
  const apiDocsHref = API_BASE_URL ? apiUrl("/scalar").toString() : "/scalar";
  let menuOpen = false;
  let menuClosing = false;
  let menuDragging = false;
  let menuDrag = 0;
  let menuMotion = 0;
  let menuPanel: HTMLDivElement;
  let menuToggle: HTMLButtonElement;
  $: activeLocale = $locale;

  async function openMenu() {
    menuMotion += 1;
    menuClosing = false;
    menuDrag = 0;
    menuOpen = true;
    await tick();
    menuPanel.querySelector<HTMLButtonElement>("button")?.focus();
  }
  async function closeMenu() {
    if (!menuOpen || menuClosing) return;
    const motion = ++menuMotion;
    menuDragging = false;
    menuClosing = true;
    await tick();
    // Flush the closing style so we can wait for its real CSS transition.
    getComputedStyle(menuPanel).transform;
    await Promise.all(menuPanel.getAnimations().map((animation) => animation.finished.catch(() => {})));
    if (motion !== menuMotion) return;
    menuOpen = false;
    menuClosing = false;
    menuDrag = 0;
    await tick();
    menuToggle?.focus();
  }
  function swipeMenu(node: HTMLElement) {
    let start: { id: number; x: number; y: number; time: number } | null = null;
    let swiping = false;
    let suppressClick = false;
    const down = (event: PointerEvent) => {
      if (!menuOpen || menuClosing || event.pointerType === "mouse" || !event.isPrimary) return;
      start = { id: event.pointerId, x: event.clientX, y: event.clientY, time: event.timeStamp };
      swiping = false;
      suppressClick = false;
    };
    const move = (event: PointerEvent) => {
      if (!start || event.pointerId !== start.id) return;
      const dx = event.clientX - start.x, dy = event.clientY - start.y;
      if (!swiping) {
        if (Math.abs(dy) > 10 && Math.abs(dy) > Math.abs(dx)) { start = null; return; }
        if (dx >= -10 || Math.abs(dx) <= Math.abs(dy) * 1.2) return;
        swiping = true;
        menuDragging = true;
        node.setPointerCapture(event.pointerId);
      }
      menuDrag = Math.min(0, dx);
      suppressClick = true;
    };
    const end = (event: PointerEvent) => {
      if (!start || event.pointerId !== start.id) return;
      const dx = event.clientX - start.x;
      const velocity = dx / Math.max(1, event.timeStamp - start.time);
      const shouldClose = event.type !== "pointercancel" && swiping &&
        (dx < -Math.min(90, node.clientWidth * .25) || (dx < -24 && velocity < -.5));
      start = null;
      menuDragging = false;
      if (node.hasPointerCapture(event.pointerId)) node.releasePointerCapture(event.pointerId);
      if (shouldClose) void closeMenu();
      else menuDrag = 0;
    };
    const click = (event: MouseEvent) => {
      if (suppressClick) { event.preventDefault(); event.stopPropagation(); suppressClick = false; }
    };
    node.addEventListener("pointerdown", down);
    node.addEventListener("pointermove", move);
    node.addEventListener("pointerup", end);
    node.addEventListener("pointercancel", end);
    node.addEventListener("click", click, true);
    return { destroy() {
      node.removeEventListener("pointerdown", down);
      node.removeEventListener("pointermove", move);
      node.removeEventListener("pointerup", end);
      node.removeEventListener("pointercancel", end);
      node.removeEventListener("click", click, true);
    } };
  }
  function onKeyDown(event: KeyboardEvent) {
    if (!menuOpen) return;
    if (event.key === "Escape") { event.preventDefault(); closeMenu(); }
    if (event.key !== "Tab") return;
    const items = Array.from(menuPanel.querySelectorAll<HTMLElement>("a[href], button"));
    const first = items[0], last = items.at(-1);
    if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
    else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
  }
  function closeOnBackdrop(event: MouseEvent) {
    if (event.target === event.currentTarget) closeMenu();
  }
  function portalActions(node: HTMLDivElement, open: boolean) {
    const anchor = document.createComment("header links");
    node.before(anchor);
    const mobile = window.matchMedia("(max-width: 640px)");
    let unlockPage: (() => void) | undefined;
    let restoreBackground: (() => void) | undefined;
    const place = () => {
      if (mobile.matches) {
        if (node.parentElement !== document.body) document.body.appendChild(node);
        if (open) {
          unlockPage ??= lockPageScroll();
          if (!restoreBackground) {
            const shell = document.querySelector<HTMLElement>(".appShell");
            if (shell) {
              const wasInert = shell.inert;
              shell.inert = true;
              restoreBackground = () => { shell.inert = wasInert; };
            }
          }
        } else {
          unlockPage?.(); unlockPage = undefined;
          restoreBackground?.(); restoreBackground = undefined;
        }
      } else {
        if (node.previousSibling !== anchor) anchor.after(node);
        unlockPage?.(); unlockPage = undefined;
        restoreBackground?.(); restoreBackground = undefined;
        if (open) { menuMotion += 1; menuOpen = false; menuClosing = false; menuDrag = 0; }
      }
    };
    place(); mobile.addEventListener("change", place);
    return {
      update(value: boolean) { open = value; place(); },
      destroy() {
        mobile.removeEventListener("change", place);
        unlockPage?.(); restoreBackground?.(); node.remove(); anchor.remove();
      }
    };
  }
  function toggleTheme() {
    const root = document.documentElement;
    const theme = root.dataset.theme === "dark" ? "light" : "dark";
    root.dataset.theme = theme;
    root.dataset.themeManual = "true";
    try { localStorage.setItem("pay3flow-theme", theme); } catch {}
  }
</script>

<svelte:window on:keydown={onKeyDown} />
<header class="header">
  <div class="inner">
    <button class="menuToggle" bind:this={menuToggle} type="button" aria-haspopup="dialog" aria-controls="header-menu" aria-expanded={menuOpen} aria-label={t("Open menu", {}, activeLocale)} on:click={openMenu}><img src={menuIcon} alt="" width="24" height="24" /></button>
    <div class="brand"><img class="logo" src="/icons/assets/pay3flow_logo.svg" alt="" width="34" height="34" /><span class="wordmark">Pay3Flow</span></div>
    <div class="actionsBackdrop" class:menuOpen class:menuClosing use:portalActions={menuOpen} role="presentation" on:mousedown={closeOnBackdrop}>
      <div id="header-menu" class="actionsPanel" class:menuDragging style:--menu-drag={`${menuDrag}px`} bind:this={menuPanel} use:swipeMenu role={menuOpen ? "dialog" : undefined} aria-modal={menuOpen ? "true" : undefined} aria-label={menuOpen ? t("Menu", {}, activeLocale) : undefined}>
        <div class="actionsHeader"><strong>{t("Menu", {}, activeLocale)}</strong><button type="button" class="menuClose" aria-label={t("Close menu", {}, activeLocale)} on:click={closeMenu}><span aria-hidden="true">❯</span></button></div>
        <nav class="actions" aria-label={t("Menu", {}, activeLocale)}>
          {#if shareState}<button type="button" class="shareButton" aria-label={t("Share exchange", {}, activeLocale)} title={t("Share exchange", {}, activeLocale)} aria-haspopup="dialog" aria-expanded={shareOpen} aria-busy={shareLoading} on:click={openShare}><img src="/icons/ui/share.png" alt="" width="20" height="20" /><span class="mobileActionLabel">{t("Share exchange", {}, activeLocale)}</span></button>{/if}
          <a class="apiDocsLink" href={apiDocsHref} target="_blank" rel="noreferrer noopener" aria-label={t("Open API documentation", {}, activeLocale)} on:click={closeMenu}>API DOCS</a>
          <a class="telegramLink" href="https://t.me/pay3flow" target="_blank" rel="noreferrer noopener" aria-label={t("Open Pay3Flow Telegram channel", {}, activeLocale)} on:click={closeMenu}><img src="/icons/assets/telegram-messenger.png" alt="" width="20" height="20" /><span class="mobileActionLabel">Telegram</span></a>
          <a class="githubLink" href="https://github.com/Flow3Pay/pay3flow" target="_blank" rel="noreferrer noopener" aria-label={t("Open Pay3Flow on GitHub", {}, activeLocale)} on:click={closeMenu}><svg viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M12 .7a11.3 11.3 0 0 0-3.58 22.02c.57.1.78-.25.78-.55v-2.16c-3.18.7-3.85-1.34-3.85-1.34-.52-1.32-1.27-1.67-1.27-1.67-1.04-.71.08-.7.08-.7 1.15.08 1.76 1.18 1.76 1.18 1.02 1.75 2.68 1.24 3.34.95.1-.74.4-1.24.73-1.53-2.54-.29-5.2-1.27-5.2-5.65 0-1.25.45-2.26 1.18-3.06-.12-.29-.51-1.45.11-3.02 0 0 .96-.31 3.12 1.17a10.8 10.8 0 0 1 5.68 0c2.16-1.48 3.12-1.17 3.12-1.17.62 1.57.23 2.73.11 3.02.73.8 1.18 1.81 1.18 3.06 0 4.39-2.67 5.35-5.21 5.64.41.36.78 1.08.78 2.18v3.23c0 .3.2.65.79.54A11.3 11.3 0 0 0 12 .7Z" /></svg><span class="mobileActionLabel">GitHub</span></a>
        </nav>
      </div>
    </div>
    <div class="utilityActions">
      <WalletMenu />
      <button class="languageToggle" type="button" on:click={() => setLocale(cycleLocale(activeLocale))} aria-label={t("Switch language", {}, activeLocale)}><span>{localeLabel(activeLocale)}</span></button>
      <button class="themeToggle" type="button" on:click={toggleTheme} aria-label={t("Switch theme", {}, activeLocale)}><img class="moonIcon" src={moonIcon} alt="" width="20" height="20" /><img class="sunIcon" src={sunIcon} alt="" width="20" height="20" /></button>
    </div>
  </div>
</header>
{#if shareOpen && ShareDialog && sharedState}<svelte:component this={ShareDialog} state={sharedState} url={shareLink} onClose={() => shareOpen = false} />{/if}

<style>
.header { position: relative; z-index: 50; padding: 16px var(--page-gutter) 8px; }
.inner { display: flex; width: min(var(--layout-width), 100%); min-height: 58px; align-items: center; gap: 16px; margin: 0 auto; padding: 6px 8px 6px 14px; border: 1px solid var(--color-border); border-radius: 13px; background: var(--color-paper); }
.brand { display: flex; min-width: 0; align-items: center; gap: 9px; margin-right: auto; }
.logo { display: block; flex: 0 0 auto; border-radius: 9px; }
.wordmark { font-size: 17px; font-weight: 800; letter-spacing: -.045em; }
.actionsBackdrop, .actionsPanel { display: contents; }
.actions, .utilityActions { display: flex; align-items: center; gap: 8px; }
.actionsHeader, .menuToggle, .mobileActionLabel { display: none; }
.shareButton, .githubLink, .telegramLink, .themeToggle, .languageToggle, .apiDocsLink { display: grid; width: 36px; height: 36px; place-items: center; border: 1px solid var(--color-border); border-radius: 9px; background: var(--color-panel); transition: background .16s ease; }
.shareButton:hover, .githubLink:hover, .telegramLink:hover, .themeToggle:hover, .languageToggle:hover, .apiDocsLink:hover { background: var(--color-accent-soft); }
.shareButton img, .githubLink svg, .telegramLink img { display: block; width: 20px; height: 20px; }
.languageToggle, .apiDocsLink { color: var(--color-text-soft); font-size: 12px; font-weight: 800; }
.apiDocsLink { width: auto; padding: 0 12px; white-space: nowrap; }
.shareButton img { filter: brightness(0); }
:global(html[data-theme="dark"]) .shareButton img { filter: brightness(0) invert(1); }
.themeToggle { position: relative; }
.themeToggle img { position: absolute; }
.moonIcon { filter: brightness(0); }
.sunIcon { display: none; }
:global(html[data-theme="dark"]) .moonIcon { display: none; }
:global(html[data-theme="dark"]) .sunIcon { display: block; filter: brightness(0) invert(1); }
@media (max-width: 640px) {
  .header { padding-top: 12px; }
  .inner { gap: 8px; padding: 6px; }
  .wordmark { font-size: 15px; }
  .brand { gap: 6px; }
  .logo { width: 28px; height: 28px; }
  .utilityActions { gap: 8px; }
  .themeToggle, .languageToggle { width: 44px; height: 44px; }
  .menuToggle, .menuClose { display: grid; width: 44px; height: 44px; flex: 0 0 auto; place-items: center; border: 1px solid var(--color-border); border-radius: 9px; background: var(--color-panel); }
  .menuToggle img { filter: brightness(0); }
  :global(html[data-theme="dark"]) .menuToggle img { filter: brightness(0) invert(1); }
  .actionsBackdrop { position: fixed; inset: 0; z-index: 1200; display: flex; height: 100dvh; background: rgba(8, 11, 8, .52); touch-action: none; visibility: hidden; pointer-events: none; opacity: 0; transition: opacity .38s ease, visibility 0s .38s; }
  .actionsBackdrop.menuOpen { visibility: visible; pointer-events: auto; opacity: 1; transition-delay: 0s; }
  .actionsBackdrop.menuClosing { opacity: 0; pointer-events: none; }
  .actionsPanel { display: block; width: min(320px, calc(100% - 48px)); height: 100%; overflow-y: auto; padding: max(16px, env(safe-area-inset-top)) 16px max(16px, env(safe-area-inset-bottom)); border-right: 1px solid var(--color-border); background: var(--color-paper); touch-action: pan-y; transform: translateX(calc(-100% - 1px)); transition: transform .38s cubic-bezier(.22, 1, .36, 1); }
  .menuOpen .actionsPanel { transform: translateX(var(--menu-drag, 0px)); }
  .menuClosing .actionsPanel { transform: translateX(calc(-100% - 1px)); }
  .actionsPanel.menuDragging { transition: none; }
  .menuClose { border: 0; background: transparent; color: var(--color-text-soft); }
  .menuClose span { display: block; font-size: 21px; line-height: 1; -webkit-text-stroke: .55px currentColor; transform: rotate(180deg); }
  .actionsHeader { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding-bottom: 16px; border-bottom: 1px solid var(--color-border); }
  .actionsHeader strong { font-size: 16px; }
  .actions { display: grid; gap: 12px; padding-top: 20px; }
  .shareButton, .githubLink, .telegramLink, .apiDocsLink { display: flex; width: 100%; height: 48px; align-items: center; gap: 12px; padding: 0 12px; font-size: 14px; }
  .mobileActionLabel { display: inline; }
}
@media (prefers-reduced-motion: reduce) { .actionsPanel, .actionsBackdrop { transition: none; } }
@media (max-width: 360px) { .logo { display: none; } }
</style>
