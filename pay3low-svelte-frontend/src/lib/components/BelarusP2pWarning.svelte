<script lang="ts">
  import { onDestroy, tick } from "svelte";
  import { warningIcon } from "$lib/icons";
  import { locale, t } from "$lib/i18n";

  export let open = false;
  export let onClose: () => void;

  const DECREE_URL = "https://president.gov.by/ru/documents/ukaz-no-367-ot-17-sentabra-2024-g";
  let dialog: HTMLDivElement;
  let active = false;
  let previousOverflow = "";
  $: activeLocale = $locale;

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") onClose();
  }

  async function activate() {
    if (active || typeof document === "undefined") return;
    active = true;
    previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    window.addEventListener("keydown", handleKeydown);
    await tick();
    dialog?.focus();
  }

  function deactivate() {
    if (!active || typeof document === "undefined") return;
    active = false;
    document.body.style.overflow = previousOverflow;
    window.removeEventListener("keydown", handleKeydown);
  }

  function closeFromBackdrop(event: MouseEvent) {
    if (event.target === event.currentTarget) onClose();
  }

  $: if (open) void activate();
  $: if (!open) deactivate();
  onDestroy(deactivate);
</script>

{#if open}
  <div class="warningBackdrop" role="presentation" on:mousedown={closeFromBackdrop}>
    <div class="warningDialog" bind:this={dialog} role="dialog" aria-modal="true" aria-labelledby="belarus-p2p-warning-title" tabindex="-1">
      <button type="button" class="closeButton" on:click={onClose} aria-label={t("Close warning", {}, activeLocale)}>
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="m7 7 10 10m0-10L7 17" stroke="currentColor" stroke-width="2" stroke-linecap="round" /></svg>
      </button>
      <span class="warningMark" aria-hidden="true"><img src={warningIcon} alt="" width="28" height="28" /></span>
      <p class="eyebrow">{t("Important for users in Belarus", {}, activeLocale)}</p>
      <h2 id="belarus-p2p-warning-title">{t("Restriction on P2P crypto transactions", {}, activeLocale)}</h2>
      <p>{t("Decree 367 Belarus crypto restriction", {}, activeLocale)}</p>
      <p class="scope">{t("Pay3Flow legal status warning", {}, activeLocale)}</p>
      <a class="decreeLink" href={DECREE_URL} target="_blank" rel="noopener noreferrer">
        <span>{t("Read Decree 367 on the official portal", {}, activeLocale)}</span>
        <span aria-hidden="true">↗</span>
      </a>
    </div>
  </div>
{/if}

<style>
  .warningBackdrop {
    position: fixed;
    inset: 0;
    z-index: 1300;
    display: grid;
    place-items: center;
    padding: 22px;
    background: rgba(13, 15, 12, 0.72);
    backdrop-filter: blur(14px) saturate(115%);
    -webkit-backdrop-filter: blur(14px) saturate(115%);
    animation: backdropIn 0.18s ease-out;
  }

  .warningDialog {
    position: relative;
    width: min(100%, 520px);
    padding: 34px;
    border: 1px solid rgba(237, 190, 44, 0.38);
    border-radius: 28px;
    outline: none;
    background: #faf9f3;
    box-shadow: 0 35px 110px rgba(0, 0, 0, 0.38);
    color: #20221f;
    animation: dialogIn 0.26s cubic-bezier(0.22, 1, 0.36, 1);
  }

  .closeButton {
    position: absolute;
    top: 18px;
    right: 18px;
    display: grid;
    width: 38px;
    height: 38px;
    place-items: center;
    border: 0;
    border-radius: 12px;
    background: rgba(31, 34, 29, 0.06);
    color: #5d6258;
    cursor: pointer;
  }

  .closeButton:hover { background: rgba(31, 34, 29, 0.1); }

  .warningMark {
    display: grid;
    width: 54px;
    height: 54px;
    place-items: center;
    margin-bottom: 20px;
    border: 1px solid rgba(237, 190, 44, 0.42);
    border-radius: 17px;
    background: rgba(255, 199, 0, 0.14);
  }

  .warningMark img {
    filter: brightness(0) saturate(100%) invert(76%) sepia(91%) saturate(1160%) hue-rotate(355deg) brightness(101%) contrast(99%);
  }

  .eyebrow {
    margin: 0 0 8px;
    color: #9a7100;
    font-size: 10px;
    font-weight: 850;
    letter-spacing: 0.09em;
    text-transform: uppercase;
  }

  h2 {
    max-width: 410px;
    margin: 0 0 18px;
    font-size: clamp(24px, 5vw, 32px);
    letter-spacing: -0.045em;
    line-height: 1.08;
  }

  p {
    margin: 0;
    color: #50554d;
    font-size: 14px;
    line-height: 1.65;
  }

  .scope {
    margin-top: 13px;
    color: #777b73;
    font-size: 12px;
  }

  .decreeLink {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 20px;
    margin-top: 24px;
    padding: 15px 17px;
    border-radius: 15px;
    background: #20231f;
    color: #fff;
    font-size: 12px;
    font-weight: 750;
    text-decoration: none;
    transition: background 0.15s ease, transform 0.15s ease;
  }

  .decreeLink:hover { background: #343832; transform: translateY(-1px); }

  @keyframes backdropIn { from { opacity: 0; } }
  @keyframes dialogIn { from { opacity: 0; transform: translateY(16px) scale(0.98); } }

  @media (max-width: 560px) {
    .warningBackdrop { align-items: end; padding: 10px; }
    .warningDialog { padding: 28px 22px 22px; border-radius: 24px; }
    .warningMark { width: 48px; height: 48px; }
  }

  @media (prefers-reduced-motion: reduce) {
    .warningBackdrop,
    .warningDialog { animation: none; }
  }

  :global(html[data-theme="dark"]) .warningDialog {
    border-color: rgba(255, 199, 0, 0.38);
    background: #20221f;
    color: #fff;
  }

  :global(html[data-theme="dark"]) .warningDialog p { color: rgba(255, 255, 255, 0.72); }
  :global(html[data-theme="dark"]) .warningDialog .scope { color: rgba(255, 255, 255, 0.5); }
  :global(html[data-theme="dark"]) .closeButton { background: rgba(255, 255, 255, 0.08); color: rgba(255, 255, 255, 0.68); }
  :global(html[data-theme="dark"]) .decreeLink { background: #ffc700; color: #1d201b; }
</style>
