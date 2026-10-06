<script lang="ts">
  import { API_BASE_URL, apiUrl } from "$lib/api";
  import { locale, localeLabel, setLocale, t, type Locale } from "$lib/i18n";
  import { lockPageScroll } from "$lib/page-scroll-lock";
  import { onDestroy } from "svelte";
  import { shareUrl, SHARE_IMAGE_VERSION, type ShareState } from "$lib/share";
  export let shareState: ShareState | null = null;
  const copyIcon = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAABt0lEQVR4AcyULU8DQRCG2WoSCCgS0gYLBgWiCgl4FAbDxx8gKQYcBoco4PkDCBAYTAVBoEAhaEOC4EsQkBzPbGYud5ftfSQloZn3Zmf2nXk7u7mrDf3x738IRFE0AfbANXgDWfsgcQuOswdSOAFF0xRdgl0wB8ZA1kZJzIJ1+BE+tkIBmDtgBpQ2RPaNXEZg0cgV/LJxywicQb5S3OAfwRfIs7pt9hVgTCck59yac27BAeJ5/BQYZn0A+tmIbaQEaFoHbdCF8INPmea6JNusDxHyf4J1X4sFKJKxOjA3gaxxQZM94XS0ZiPI0mQsQNwCk6CsCbfFFCcUfIOgJQWWgoz8pNW8ZGjvFnsBRpWzlNEtX9bLnUnta6bgwWIvwJipt882y3it/cxwLyz2AhYMyN/RJ/4mDUpAXkA591OarzDVM97bQARouA3GwSq495314QX0kjVVzRXVegFU5ZJ71Vp7dk9rfRB6eAHdOFdfxRXWJAXkG/5UobtwpSa3JBZgVDmiJuwjIGtc0GRPOE2tCZIsGQtIQgrAFmgQ1/Ap01yDpHBEiFS+pQSSVJrIxSdTQ6FcihAIfgEAAP//VzkXCgAAAAZJREFUAwA+5Z8xrrhgWAAAAABJRU5ErkJggg==";
  const moonIcon = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAACWklEQVR4AbTVTYhNYRzH8TPCRs0kLxsWIvIywkrykvISTZRkbGiSQsqCIkuWyobkpZkiL5PQ2Eyk2BBGErOaZCMz0giRrEyNz/90z3XvzL23e6Zm+n3v/zz/5/n/f8957plzJyTj/FfTYHh4eDM60Il5Y9lLRQPN5uOyho+wH7GuQ8ytKCwr0ni7xHMcxGOsxW68QW6VGWi+TofrmIGzDQ0Nm8QpCL2Oj7wUDTRvVnwVTRqHjrsOLYoPvEJuFQ1U7sFc7ESplhoMcvwo5lZqYPcTVR5Ap0ZdYqmmG/RjTEoNVLZhKi5hpOILXzAyWe84M8jO+VOFwndyje4yW2NYvzKD2P0fx1PJoLfQLp6owmX9odRgoFIZ00H5CzjnLhaKuVRq8LdaJZMj5n7hIXIpM4jCZjuMx7Rag30m5lgzgOWu61Jm8KywuqUQRwV3cV9yI2bhLZMTmO26plIDxS+t+ow1qCrrnpichms4g34mvWjHqRLWm0uVGqRXSXJHbLWo6l2YT5j8QBzXLuMw+SbGy/CwuA0Rb4upigaKjsnE+6aLSfxnG1aX9fdwEhvQiJlW78VX9CFV0SAdJckWcTKeIpdsarGCOIUl4kWkKjOwi5+yq7FKwW/EC9CwtqzbakXWvFWfu8apygwiY/KFOAm3cENxfHktYhyB1H/JrcRNmQdoQllz42SUQSSZDOGQ66BV7MagZu/Rgw/4LteDZYjfjhVqijuXS1XRIJ3xoeAK4myDeJ1Hwy+mwvC0GE9NNI5fv3iapMpV0yBbyqQP7WjDDhzFeXRjKFtXKf4DAAD///Lx6McAAAAGSURBVAMASQPNMX2ya7kAAAAASUVORK5CYII=";
  const sunIcon = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAACD0lEQVR4AbSTPS8FQRSG92o0+Ae0EkKDhk58NJQUoiASlUoQKolCoySESCQaCdEp0BGREBQK4W+goJDreSdnstmdXXe3cHOenDNn3nfO7t7duuiff6UGVKvVWVHmmkoNKHOw15YaUKlU9oQ3F8nBAB7BBSxlmem3ipy9ZfbO03vBAARX8AkuMOm535DfabwK1aDeLGsf8lz7hc/BAB7BOmxLwCEP5F1ogn2YMFSrt2uaCM8WrLOfiGCA38V4TN0Fl9CLeR6OjHn1QHtdpmUZRuYADCNIx0BXNcyhun2WcagHw3S2YMw8lMnIHIBkBZ5gAWqFNNLKE2jzBnSivOcKv8l/hmnuEclDSoYbwO3pTVnUFnUbuQHuoGhI22DeiLwI7g1zA1Kn/Ni65tWbTslrvVc9hxvAbeoL3VCH+o38AT1QNKT9MK9e2Q3qPZndABUpnlkPcJt6VJT5YZoBFPKQkpE34BBZB5xCrZBGWnkCbeYAu71H1ENc4SbUUydCPdikOQSP5qFMRuYASTB0k09gDm45bAemjB31QHsnpmUZRjCAA1ZgWlKM4+RR+IJJODBUqzdqGr2aM/iW2U9EMIDdfmgGFxxwBn3QSKNdqAb1zlj7aKGQlxRHMADjIKzFkrii/yLiTlzRXwX9H3GTKhhALzd4BPri3ReaK0ptlBqQ8hZalhrAI9AX777QQqdHUfQLAAD//91ClIsAAAAGSURBVAMAR3zSMQ+aPXkAAAAASUVORK5CYII=";
  const menuIcon = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAAAdElEQVR4AexTwQ3AIAgsXaSzdCpncKrO0knoSYq/xpKoD3OEUx4I8S63b4ODC5oEL0SRqibgBjwvFBGg3bLMSM7dPIpEJAMH4HmiiADtlmVGnv8D39j7rhpAHopsCuOgyPrlbn2DTo5ZsRot9ux/Nxc0uXoAAAD//8ndUOoAAAAGSURBVAMAa9HwMZNspewAAAAASUVORK5CYII=";

  const apiDocsHref = API_BASE_URL ? apiUrl("/scalar").toString() : "/scalar";
  let menuOpen = false;
  let languageOpen = false;
  let languagePicker: HTMLDivElement;
  let languageToggle: HTMLButtonElement;
  const languages: { code: Locale; name: string; flag: string }[] = [
    { code: "en", name: "English", flag: "/icons/flags/us.svg" },
    { code: "ru", name: "Русский", flag: "/icons/flags/ru.svg" },
    { code: "hy", name: "Հայերեն", flag: "/icons/flags/am.svg" }
  ];
  let menuPanel: HTMLDivElement;
  let menuDragging = false;
  let menuDragStartY = 0;
  let menuDragDistance = 0;
  let shareOpen = false;
  let shareLink = "";
  let sharePreview = "";
  let shareCopied = false;
  let shareError = "";
  let unlockShare: (() => void) | undefined;

  function openShare() {
    closeMenu();
    if (!shareState) return;
    shareLink = shareUrl(location.origin, shareState).toString();
    const preview = new URL("/share-image.png", location.origin);
    preview.searchParams.set("v", SHARE_IMAGE_VERSION);
    preview.searchParams.set("from", shareState.source);
    preview.searchParams.set("to", shareState.target);
    const shared = new URL(shareLink);
    for (const key of ["amount", "receive"]) {
      const value = shared.searchParams.get(key);
      if (value) preview.searchParams.set(key, value);
    }
    sharePreview = preview.toString();
    shareCopied = false;
    shareError = "";
    shareOpen = true;
    unlockShare ??= lockPageScroll();
  }
  function closeShare() {
    shareOpen = false;
    unlockShare?.();
    unlockShare = undefined;
  }
  async function copyShareLink() {
    try {
      await navigator.clipboard.writeText(shareLink);
      shareCopied = true;
      shareError = "";
    } catch {
      shareError = t("Select and copy the link", {}, activeLocale);
    }
  }
  onDestroy(() => unlockShare?.());

  function closeMenu() { menuOpen = false; languageOpen = false; }
  function closeOnBackdrop(event: MouseEvent) { if (event.target === event.currentTarget) closeMenu(); }
  function closeLanguageOnOutsideClick(event: PointerEvent) {
    if (languageOpen && !languagePicker?.contains(event.target as Node)) languageOpen = false;
  }
  function onKeyDown(event: KeyboardEvent) {
    if (event.key !== "Escape") return;
    if (shareOpen) closeShare();
    else if (languageOpen) { languageOpen = false; languageToggle?.focus(); }
    else if (menuOpen) closeMenu();
  }
  function startMenuDrag(event: PointerEvent) {
    menuDragging = true;
    menuDragStartY = event.clientY;
    menuDragDistance = 0;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  }
  function moveMenuDrag(event: PointerEvent) {
    if (!menuDragging) return;
    menuDragDistance = Math.max(0, event.clientY - menuDragStartY);
    menuPanel?.style.setProperty("--menu-drag", `${menuDragDistance}px`);
  }
  function endMenuDrag() {
    if (!menuDragging) return;
    menuDragging = false;
    if (menuDragDistance > 72) closeMenu();
    else menuPanel?.style.removeProperty("--menu-drag");
  }
  function portalActions(node: HTMLDivElement, open: boolean) {
    const anchor = document.createComment("header actions");
    node.before(anchor);
    const mobile = window.matchMedia("(max-width: 640px)");
    let unlockPage: (() => void) | undefined;
    const place = () => {
      if (mobile.matches) {
        if (node.parentElement !== document.body) document.body.appendChild(node);
        if (open) unlockPage ??= lockPageScroll();
        else { unlockPage?.(); unlockPage = undefined; }
      } else {
        if (node.previousSibling !== anchor) anchor.after(node);
        unlockPage?.();
        unlockPage = undefined;
        if (open) closeMenu();
      }
    };
    place();
    mobile.addEventListener("change", place);
    return {
      update(value: boolean) { open = value; place(); },
      destroy() {
        mobile.removeEventListener("change", place);
        unlockPage?.();
        node.remove();
        anchor.remove();
      }
    };
  }

  function toggleTheme() {
    const root = document.documentElement;
    const nextTheme = root.dataset.theme === "dark" ? "light" : "dark";
    root.dataset.theme = nextTheme;
    root.dataset.themeManual = "true";
    try { localStorage.setItem("pay3flow-theme", nextTheme); } catch {}
    closeMenu();
  }

  $: activeLocale = $locale;
  $: activeLanguage = languages.find((language) => language.code === activeLocale) ?? languages[0];
  function chooseLocale(next: Locale) {
    setLocale(next);
    languageOpen = false;
    closeMenu();
  }
</script>

<svelte:window on:keydown={onKeyDown} on:pointerdown={closeLanguageOnOutsideClick} />

<header class="header">
  <div class="inner">
    <div class="brand">
      <span class="logo" aria-hidden="true">
        <img src="/icons/assets/pay3flow_logo.svg" alt="" width="34" height="34" decoding="async" />
      </span>
      <span class="wordmark">Pay3Flow</span>
    </div>
    <div class="actionsBackdrop" class:menuOpen use:portalActions={menuOpen} role="presentation" on:mousedown={closeOnBackdrop}>
      <div class="actionsPanel" bind:this={menuPanel} role={menuOpen ? "dialog" : undefined} aria-modal={menuOpen ? "true" : undefined} aria-label={menuOpen ? t("Menu", {}, activeLocale) : undefined} on:mousedown|stopPropagation>
        <div class="actionsHeader"><button type="button" class="menuHandle" aria-label={t("Close menu", {}, activeLocale)} on:pointerdown={startMenuDrag} on:pointermove={moveMenuDrag} on:pointerup={endMenuDrag} on:pointercancel={endMenuDrag} on:keydown={(event) => { if (event.key === "Enter" || event.key === " ") closeMenu(); }}><span aria-hidden="true"></span></button><strong>{t("Menu", {}, activeLocale)}</strong></div>
        <div class="actions">
          <a class="apiDocsLink" href={apiDocsHref} target="_blank" rel="noreferrer noopener" aria-label="Open API documentation" on:click={closeMenu}>API DOCS</a>
          <button class="shareButton" type="button" on:click={openShare} disabled={!shareState} aria-label={t("Share bridge", {}, activeLocale)}>{t("Share", {}, activeLocale)}</button>
          <div class="languagePicker" bind:this={languagePicker}>
            <button class="languageToggle" bind:this={languageToggle} type="button" on:click={() => languageOpen = !languageOpen} aria-label={t("Switch language", {}, activeLocale)} aria-haspopup="true" aria-expanded={languageOpen} aria-controls="language-options" title={t("Switch language", {}, activeLocale)}><img class="languageFlag" src={activeLanguage.flag} alt="" width="16" height="16" aria-hidden="true" /><span class="localeCode">{localeLabel(activeLocale)}</span><span class="mobileActionLabel">{t("Switch language", {}, activeLocale)}</span><span class="languageChevron" aria-hidden="true"></span></button>
            {#if languageOpen}
              <div class="languageOptions" id="language-options" role="group" aria-label={t("Switch language", {}, activeLocale)}>
                {#each languages as language}
                  <button type="button" class="languageOption" class:selected={activeLocale === language.code} aria-pressed={activeLocale === language.code} lang={language.code} on:click={() => chooseLocale(language.code)}><img class="languageFlag" src={language.flag} alt="" width="18" height="18" aria-hidden="true" /><span class="languageOptionName">{language.name}</span><span class="languageOptionCode">{localeLabel(language.code)}</span></button>
                {/each}
              </div>
            {/if}
          </div>
          <button class="themeToggle" type="button" on:click={toggleTheme} aria-label={t("Switch theme", {}, activeLocale)} title={t("Switch theme", {}, activeLocale)}><span class="themeIcon"><img class="moonIcon" src={moonIcon} alt="" width="24" height="24" decoding="async" /><img class="sunIcon" src={sunIcon} alt="" width="24" height="24" decoding="async" /></span><span class="mobileActionLabel">{t("Switch theme", {}, activeLocale)}</span></button>
          <a class="telegramLink" href="https://t.me/+-lq4m5E_aT4xM2Y6" target="_blank" rel="noreferrer noopener" aria-label="Open Pay3Flow Telegram channel" title="Telegram" on:click={closeMenu}><img src="/icons/assets/telegram-messenger.png" alt="" width="20" height="20" decoding="async" /><span class="mobileActionLabel">Telegram</span></a>
          <a class="githubLink" href="https://github.com/Flow3Pay/pay3flow" target="_blank" rel="noreferrer noopener" aria-label="Open Pay3Flow on GitHub" on:click={closeMenu}><svg viewBox="0 0 24 24" aria-hidden="true" focusable="false"><path fill="currentColor" d="M12 .7a11.3 11.3 0 0 0-3.58 22.02c.57.1.78-.25.78-.55v-2.16c-3.18.7-3.85-1.34-3.85-1.34-.52-1.32-1.27-1.67-1.27-1.67-1.04-.71.08-.7.08-.7 1.15.08 1.76 1.18 1.76 1.18 1.02 1.75 2.68 1.24 3.34.95.1-.74.4-1.24.73-1.53-2.54-.29-5.2-1.27-5.2-5.65 0-1.25.45-2.26 1.18-3.06-.12-.29-.51-1.45.11-3.02 0 0 .96-.31 3.12 1.17a10.8 10.8 0 0 1 5.68 0c2.16-1.48 3.12-1.17 3.12-1.17.62 1.57.23 2.73.11 3.02.73.8 1.18 1.81 1.18 3.06 0 4.39-2.67 5.35-5.21 5.64.41.36.78 1.08.78 2.18v3.23c0 .3.2.65.79.54A11.3 11.3 0 0 0 12 .7Z" /></svg><span class="mobileActionLabel">GitHub</span></a>
        </div>
      </div>
    </div>
    <button class="menuToggle" type="button" aria-haspopup="dialog" aria-expanded={menuOpen} aria-label={t("Open menu", {}, activeLocale)} title={t("Open menu", {}, activeLocale)} on:click={() => menuOpen = true}><img src={menuIcon} alt="" width="24" height="24" decoding="async" /></button>
  </div>
</header>

{#if shareOpen}
  <div class="shareBackdrop" role="presentation" on:mousedown={(event) => { if (event.target === event.currentTarget) closeShare(); }}>
    <div class="shareDialog" role="dialog" aria-modal="true" aria-label={t("Share bridge", {}, activeLocale)}>
      <div class="shareHeading">
        <span class="shareMark" aria-hidden="true"><img src="/icons/assets/pay3flow_logo.svg" alt="" width="30" height="30" /></span>
        <div><span class="shareEyebrow">Pay3Flow</span><h2>{t("Share bridge", {}, activeLocale)}</h2></div>
        <button class="shareClose" type="button" on:click={closeShare} aria-label={t("Close share dialog", {}, activeLocale)}>×</button>
      </div>
      <p>{t("Send this link to open the saved route. Rates refresh on your timer.", {}, activeLocale)}</p>
      <div class="sharePreview"><img src={sharePreview} alt={t("Bridge preview", {}, activeLocale)} /></div>
      <label for="share-link">{t("Link", {}, activeLocale)}</label>
      <div class="shareLinkField">
        <input id="share-link" value={shareLink} readonly on:focus={(event) => event.currentTarget.select()} />
        <button class="shareCopy" class:copied={shareCopied} type="button" on:click={copyShareLink} aria-label={t(shareCopied ? "Copied" : "Copy link", {}, activeLocale)} title={t(shareCopied ? "Copied" : "Copy link", {}, activeLocale)}><img src={copyIcon} alt="" width="20" height="20" /></button>
      </div>
      {#if shareCopied}<span class="shareSuccess" role="status">{t("Copied", {}, activeLocale)}</span>{/if}
      {#if shareError}<span class="shareError" role="status">{shareError}</span>{/if}
    </div>
  </div>
{/if}

<style>
.header {
  position: relative;
  z-index: 50;
  width: 100%;
  padding: 18px 24px;
}

.inner {
  display: grid;
  width: 100%;
  min-height: 66px;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 24px;
  margin: 0 auto;
  padding: 8px 10px 8px 18px;
  border: 1px solid rgba(255, 255, 255, 0.7);
  border-radius: 22px;
  background: rgba(255, 255, 255, 0.72);
  box-shadow: 0 9px 30px rgba(31, 34, 29, 0.07);
  backdrop-filter: blur(24px) saturate(150%);
  -webkit-backdrop-filter: blur(24px) saturate(150%);
}

.brand,
.actions,
.profile {
  display: flex;
  align-items: center;
}

.brand {
  width: fit-content;
  gap: 9px;
}

.logo {
  display: grid;
  width: 34px;
  height: 34px;
  overflow: hidden;
  place-items: center;
  border-radius: 11px;
  background: transparent;
  box-shadow: none;
}

.logo img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.wordmark {
  font-size: 17px;
  font-weight: 800;
  letter-spacing: -0.045em;
}

.actions {
  justify-content: flex-end;
  gap: 9px;
}

.actionsBackdrop,
.actionsPanel {
  display: contents;
}

.actionsHeader,
.menuToggle,
.mobileActionLabel {
  display: none;
}

.themeIcon {
  position: relative;
  display: grid;
  width: 20px;
  height: 20px;
  place-items: center;
}

.githubLink,
.telegramLink,
.themeToggle,
.languageToggle,
.apiDocsLink,
.shareButton {
  display: grid;
  width: 36px;
  height: 36px;
  place-items: center;
  border: 1px solid var(--color-border);
  border-radius: 11px;
  background: rgba(255, 255, 255, 0.72);
  transition: border-color 0.16s ease, background 0.16s ease, transform 0.16s ease;
}

.githubLink img,
.githubLink svg,
.telegramLink img,
.themeToggle img {
  display: block;
  width: 20px;
  height: 20px;
  object-fit: contain;
}

.githubLink:hover,
.telegramLink:hover,
.themeToggle:hover,
.languageToggle:hover,
.apiDocsLink:hover,
.shareButton:hover {
  border-color: var(--color-accent-strong);
  background: var(--color-accent-soft);
  transform: translateY(-1px);
}

.themeToggle {
  position: relative;
  overflow: hidden;
}

.languageToggle {
  display: flex;
  width: 64px;
  align-items: center;
  justify-content: center;
  gap: 4px;
  padding: 0 6px;
  color: var(--color-text-soft);
  font-size: 10px;
  font-weight: 850;
  letter-spacing: 0.04em;
}

.languagePicker { position: relative; }
.languageFlag { display: block; width: 16px; height: 16px; flex: 0 0 16px; border: 1px solid var(--color-border); border-radius: 50%; object-fit: cover; }
.languageChevron { width: 6px; height: 6px; flex: 0 0 6px; margin-top: -3px; border-right: 1.5px solid currentColor; border-bottom: 1.5px solid currentColor; transform: rotate(45deg); }
.languageToggle[aria-expanded="true"] .languageChevron { margin-top: 3px; transform: rotate(225deg); }
.languageOptions { position: absolute; top: calc(100% + 8px); right: 0; z-index: 2; width: 172px; padding: 5px; border: 1px solid var(--color-border); border-radius: 12px; background: var(--color-paper); box-shadow: var(--shadow-pop); }
.languageOption { display: flex; width: 100%; min-height: 38px; align-items: center; justify-content: space-between; gap: 12px; padding: 0 10px; border-radius: 8px; color: var(--color-text); font-size: 13px; font-weight: 700; text-align: left; }
.languageOption:hover, .languageOption:focus-visible, .languageOption.selected { background: var(--color-accent-soft); }
.languageOption .languageFlag { width: 18px; height: 18px; flex-basis: 18px; }
.languageOptionName { flex: 1; }
.languageOptionCode { color: var(--color-text-soft); font-size: 10px; font-weight: 800; }

.apiDocsLink,
.shareButton {
  width: auto;
  padding: 0 12px;
  color: var(--color-text-soft);
  font-size: 10px;
  font-weight: 850;
  letter-spacing: 0.04em;
  white-space: nowrap;
}

.shareButton:disabled { opacity: 0.5; cursor: wait; }

.shareBackdrop {
  position: fixed;
  inset: 0;
  z-index: 2000;
  display: grid;
  place-items: center;
  padding: 16px;
  background: rgba(8, 11, 8, 0.68);
  backdrop-filter: blur(5px);
}

.shareDialog {
  width: min(100%, 500px);
  max-height: calc(100dvh - 32px);
  overflow-y: auto;
  padding: 20px;
  border: 1px solid var(--color-border-strong);
  border-radius: var(--radius-card);
  background: var(--color-paper);
  color: var(--color-text);
  box-shadow: var(--shadow-pop);
}

.shareHeading { display: flex; align-items: center; gap: 11px; }
.shareMark { display: grid; width: 40px; height: 40px; flex: 0 0 40px; place-items: center; border-radius: 10px; background: var(--color-panel); }
.shareMark img { display: block; width: 30px; height: 30px; }
.shareEyebrow { color: var(--color-text-faint); font-size: 10px; font-weight: 800; letter-spacing: .08em; text-transform: uppercase; }
.shareDialog h2 { margin: 1px 0 0; font-size: 19px; font-weight: 800; line-height: 1.25; }
.shareDialog p { margin: 16px 0; color: var(--color-text-soft); font-size: 13px; line-height: 1.5; }
.sharePreview { overflow: hidden; padding: 5px; border: 1px solid var(--color-border); border-radius: 12px; background: var(--color-panel); }
.sharePreview img { display: block; width: 100%; height: auto; aspect-ratio: 1200 / 630; border-radius: 8px; object-fit: cover; }
.shareDialog label { display: block; margin: 18px 0 8px; font-size: 12px; font-weight: 750; }
.shareLinkField { display: flex; min-width: 0; align-items: center; padding: 4px; border: 1px solid var(--color-border-strong); border-radius: 10px; background: var(--color-panel); transition: border-color .16s ease; }
.shareLinkField:focus-within { border-color: var(--color-accent-strong); }
.shareLinkField input { min-width: 0; flex: 1; padding: 9px 10px; border: 0; outline: 0; background: transparent; color: var(--color-text); font: 12px var(--font-mono); }
.shareCopy { display: grid; width: 38px; height: 38px; flex: 0 0 38px; place-items: center; border: 1px solid var(--color-border); border-radius: 8px; background: var(--color-paper); transition: border-color .16s ease, background .16s ease; }
.shareCopy:hover, .shareCopy.copied { border-color: var(--color-accent-strong); background: var(--color-accent-soft); }
.shareCopy img { display: block; width: 20px; height: 20px; filter: brightness(0); }
.shareClose { display: grid; width: 32px; height: 32px; flex: 0 0 32px; place-items: center; margin-left: auto; border-radius: 8px; background: var(--color-panel); color: var(--color-text-soft); font-size: 23px; line-height: 1; }
.shareClose:hover { color: var(--color-text); }
.shareSuccess, .shareError { display: block; margin-top: 8px; font-size: 12px; }
.shareSuccess { color: var(--color-good); }
.shareError { color: var(--color-danger); }
:global(html[data-theme="dark"]) .shareCopy img { filter: none; }

.themeToggle img {
  position: absolute;
  transition: opacity 0.2s ease, transform 0.25s ease;
}

.moonIcon {
  filter: brightness(0);
}

.sunIcon {
  opacity: 0;
  transform: rotate(-45deg) scale(0.65);
}

:global(html[data-theme="dark"]) .moonIcon {
  opacity: 0;
  transform: rotate(45deg) scale(0.65);
}

:global(html[data-theme="dark"]) .sunIcon {
  opacity: 1;
  filter: brightness(0) invert(1);
  transform: rotate(0) scale(1);
}

:global(html[data-theme="dark"]) .inner {
  border-color: var(--color-border-strong);
  background: rgba(25, 25, 25, 0.92);
  box-shadow: 0 14px 36px rgba(0, 0, 0, 0.25);
}

:global(html[data-theme="dark"]) .githubLink,
:global(html[data-theme="dark"]) .telegramLink,
:global(html[data-theme="dark"]) .themeToggle,
:global(html[data-theme="dark"]) .languageToggle,
:global(html[data-theme="dark"]) .apiDocsLink,
:global(html[data-theme="dark"]) .shareButton,
:global(html[data-theme="dark"]) .profile {
  border-color: var(--color-border-strong);
  background: #262626;
}

:global(html[data-theme="dark"]) .githubLink img {
  filter: invert(1) brightness(1.35);
}

:global(html[data-theme="dark"]) .menu {
  background: rgba(25, 25, 25, 0.98);
}

.connect,
.profile {
  min-height: 48px;
  border-radius: 15px;
  font-size: 12px;
  font-weight: 750;
}

.connect {
  display: inline-flex;
  align-items: center;
  gap: 9px;
  padding: 0 18px;
  background: var(--color-primary);
  color: #fff;
  box-shadow: 0 8px 18px rgba(12, 14, 12, 0.15);
  transition: transform 0.16s ease, background 0.16s ease;
}

.connect:hover {
  background: #000;
  transform: translateY(-1px);
}

.profileWrap {
  position: relative;
}

.profile {
  max-width: 230px;
  gap: 9px;
  padding: 5px 12px 5px 5px;
  border: 1px solid var(--color-border);
  background: rgba(255, 255, 255, 0.72);
}

.profileAvatar {
  display: grid;
  width: 36px;
  height: 36px;
  flex: 0 0 auto;
  place-items: center;
  border-radius: 11px;
  background: linear-gradient(145deg, var(--color-violet), #9b83ff);
  color: #fff;
  font-size: 12px;
}

.profileAddress {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.menu {
  position: absolute;
  top: calc(100% + 10px);
  right: 0;
  width: 240px;
  padding: 8px;
  border: 1px solid var(--color-border);
  border-radius: 18px;
  background: rgba(255, 255, 255, 0.96);
  box-shadow: var(--shadow-pop);
  backdrop-filter: blur(20px);
  animation: menuIn 0.18s ease;
}

.menuIdentity {
  display: flex;
  flex-direction: column;
  gap: 3px;
  padding: 11px 12px 13px;
  border-bottom: 1px solid var(--color-border);
}

.menuIdentity span {
  color: var(--color-text-faint);
  font-size: 10px;
  font-weight: 700;
  text-transform: uppercase;
}

.menuIdentity strong {
  overflow: hidden;
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.menuItem {
  width: 100%;
  margin-top: 6px;
  padding: 10px 12px;
  border-radius: 11px;
  color: var(--color-danger);
  font-size: 12px;
  font-weight: 700;
  text-align: left;
}

.menuItem:hover {
  background: rgba(212, 61, 53, 0.07);
}

@keyframes menuIn {
  from { opacity: 0; transform: translateY(-5px) scale(0.98); }
  to { opacity: 1; transform: translateY(0) scale(1); }
}

/* Shared visual language for the light workspace. Existing transitions and pulse motion remain intact. */
.header {
  padding: 16px 24px 8px;
}

.inner {
  min-height: 58px;
  padding: 6px 8px 6px 14px;
  border-color: var(--color-border-strong);
  border-radius: 13px;
  background: rgba(255, 255, 255, 0.86);
  box-shadow: 0 10px 26px rgba(55, 77, 52, 0.06);
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
}

.logo {
  width: 34px;
  height: 34px;
  border-radius: 9px;
  box-shadow: none;
}

.connect {
  min-height: 40px;
  border-radius: 9px;
  background: var(--color-primary);
  box-shadow: 0 7px 16px rgba(19, 32, 21, 0.12);
}

.connect:hover {
  background: var(--color-primary-strong);
}

.profile {
  min-height: 40px;
  border-radius: 9px;
  background: #f4f8f1;
}

.profileAvatar {
  width: 30px;
  height: 30px;
  border-radius: 8px;
  background: var(--color-primary);
  color: var(--color-accent);
}

@media (max-width: 640px) {
  .header {
    padding: 12px;
  }

  .inner {
    min-height: 58px;
    gap: 8px;
    padding: 6px 7px 6px 13px;
  }

  .profileAddress {
    display: none;
  }

  .profile {
    padding-right: 8px;
  }

  .wordmark {
    font-size: 15px;
  }

  .menuToggle {
    display: grid;
    width: 36px;
    height: 36px;
    place-items: center;
    border: 1px solid var(--color-border);
    border-radius: 11px;
    background: rgba(255, 255, 255, 0.72);
  }

  .menuToggle img {
    display: block;
    width: 20px;
    height: 20px;
    object-fit: contain;
    filter: brightness(0);
  }

  .actionsBackdrop {
    position: fixed;
    inset: 0;
    z-index: 1200;
    display: none;
    height: 100dvh;
    background: rgba(8, 11, 8, 0.52);
    touch-action: none;
  }

  .actionsBackdrop.menuOpen {
    display: grid;
    place-items: end center;
    animation: menuFadeIn 0.2s ease-out;
  }

  .actionsPanel {
    display: block;
    width: 100%;
    max-height: calc(100dvh - 16px);
    overflow-y: auto;
    padding: 10px 16px calc(18px + env(safe-area-inset-bottom));
    border: 1px solid var(--color-border);
    border-radius: 14px 14px 0 0;
    background: rgba(255, 255, 255, 0.98);
    box-shadow: var(--shadow-pop);
    touch-action: auto;
    transform: translateY(var(--menu-drag, 0px));
    transition: transform 0.24s ease;
    animation: menuSheetIn 0.24s cubic-bezier(0.22, 1, 0.36, 1);
  }

  .actionsHeader {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .actionsHeader strong {
    font-size: 13px;
    font-weight: 800;
  }

  .menuHandle {
    display: flex;
    width: 100%;
    height: 30px;
    align-items: center;
    justify-content: center;
    touch-action: none;
  }

  .menuHandle span {
    width: 38px;
    height: 5px;
    border-radius: 999px;
    background: var(--color-border-strong);
  }

  .actions {
    display: grid;
    grid-template-columns: 1fr;
    gap: 6px;
    padding-top: 14px;
  }

  .githubLink,
  .telegramLink,
  .themeToggle,
  .languageToggle,
  .apiDocsLink,
  .shareButton {
    display: flex;
    width: 100%;
    min-height: 44px;
    height: auto;
    justify-content: flex-start;
    gap: 12px;
    padding: 0 12px;
    border-radius: 10px;
    font-size: 13px;
    font-weight: 750;
    text-align: left;
  }

  .themeToggle {
    overflow: visible;
  }

  .languagePicker { width: 100%; }
  .languageToggle { justify-content: flex-start; }
  .languageChevron { margin-left: auto; }
  .languageOptions { position: static; width: 100%; margin-top: 6px; box-shadow: none; }
  .languageOption { min-height: 44px; }

  .localeCode {
    display: grid;
    width: 20px;
    height: 20px;
    flex: 0 0 20px;
    place-items: center;
    font-size: 10px;
  }

  .themeIcon,
  .telegramLink img,
  .githubLink svg {
    flex: 0 0 20px;
  }

  .mobileActionLabel {
    display: inline;
    color: var(--color-text);
    font-size: 13px;
    font-weight: 700;
  }
}

@keyframes menuFadeIn {
  from { opacity: 0; }
  to { opacity: 1; }
}

@keyframes menuSheetIn {
  from { transform: translateY(100%); }
  to { transform: translateY(0); }
}

:global(html[data-theme="dark"]) .actionsPanel {
  border-color: var(--color-border-strong);
  background: rgba(25, 25, 25, 0.98);
}

:global(html[data-theme="dark"]) .menuToggle {
  border-color: var(--color-border-strong);
  background: #262626;
}

:global(html[data-theme="dark"]) .menuToggle img {
  filter: none;
}

@media (prefers-reduced-motion: reduce) {
  .actionsBackdrop.menuOpen,
  .actionsPanel {
    animation: none;
    transition: none;
  }
}

.menu {
  top: calc(100% + 8px);
  border-color: var(--color-border-strong);
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.98);
  backdrop-filter: none;
}

</style>
