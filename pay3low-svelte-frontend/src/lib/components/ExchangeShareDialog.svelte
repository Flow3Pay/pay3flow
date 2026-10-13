<script lang="ts">
  import { tick } from 'svelte';
  import { locale, t } from '$lib/i18n';
  import { lockPageScroll } from '$lib/page-scroll-lock';
  import { exchangeShareImage, type ExchangeShareState } from '$lib/exchange-share';

  export let state: ExchangeShareState | null = null;
  export let imageUrl: string | null = null;
  export let shareTitle = "Share exchange";
  export let shareDescription = "Send this link to open the same exchange settings.";
  export let linkLabel = "Exchange link";
  export let previewAlt = "";
  export let url: string;
  export let onClose: () => void;
  let dialog: HTMLDivElement;
  let linkInput: HTMLInputElement;
  $: previewImage = imageUrl ?? (state ? exchangeShareImage(new URL(url).origin, state.source, state.target, new URL(url).searchParams) : "");
  let copied = false;
  let copyFailed = false;

  function portal(node: HTMLElement) {
    const previousFocus = document.activeElement as HTMLElement | null;
    document.body.appendChild(node);
    const shell = document.querySelector<HTMLElement>('.appShell');
    const wasInert = shell?.inert ?? false;
    if (shell) shell.inert = true;
    const unlock = lockPageScroll();
    void tick().then(() => dialog.focus());
    return { destroy() {
      if (shell) shell.inert = wasInert;
      unlock(); node.remove(); previousFocus?.focus();
    } };
  }
  async function copyLink() {
    try {
      await navigator.clipboard.writeText(url);
      copied = true; copyFailed = false;
    } catch {
      copyFailed = true;
      linkInput.focus(); linkInput.select();
    }
  }
  function keydown(event: KeyboardEvent) {
    if (event.key === 'Escape') { event.preventDefault(); onClose(); return; }
    if (event.key !== 'Tab') return;
    const items = [...dialog.querySelectorAll<HTMLElement>('button, input')];
    const first = items[0], last = items.at(-1);
    if (event.shiftKey && (document.activeElement === first || document.activeElement === dialog)) { event.preventDefault(); last?.focus(); }
    else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
  }
</script>

<svelte:window on:keydown={keydown} />
<div class="shareBackdrop" use:portal role="presentation" on:mousedown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
  <div class="shareDialog" bind:this={dialog} role="dialog" aria-modal="true" aria-labelledby="exchange-share-title" aria-describedby="exchange-share-description" tabindex="-1">
    <div class="sheetHandle" aria-hidden="true"><span></span></div>
    <div class="shareHeading"><h2 id="exchange-share-title">{t(shareTitle, {}, $locale)}</h2></div>
    <p id="exchange-share-description">{t(shareDescription, {}, $locale)}</p>
    <img class="sharePreview" src={previewImage} alt={previewAlt || `${state?.source} → ${state?.target} · Pay3Flow Bridge`} width="1200" height="630" />
    <label for="exchange-share-link">{t(linkLabel, {}, $locale)}</label>
    <div class="shareLinkRow"><input id="exchange-share-link" bind:this={linkInput} readonly value={url} on:focus={(event) => event.currentTarget.select()} /><button type="button" class="copyButton" on:click={copyLink}>{t(copied ? 'Link copied' : 'Copy link', {}, $locale)}</button></div>
    <p class="copyStatus" aria-live="polite">{copyFailed ? t('Select and copy the link manually.', {}, $locale) : copied ? t('Link copied', {}, $locale) : ''}</p>
  </div>
</div>

<style>
  .shareBackdrop { position: fixed; z-index: 1000; inset: 0; display: grid; place-items: center; padding: 16px; background: rgba(13, 15, 12, .72); backdrop-filter: blur(12px); }
  .shareDialog { position: relative; width: min(100%, 560px); max-height: calc(100dvh - 32px); overflow: auto; padding: 20px; outline: none; border: 1px solid var(--color-border-strong); border-radius: var(--radius-card); background: rgba(255, 255, 255, .96); color: var(--color-text); box-shadow: none; }
  :global(html[data-theme="dark"]) .shareDialog { background: rgba(25, 25, 25, .96); }
  .sheetHandle { display: none; }
  .shareHeading { display: flex; align-items: center; gap: 10px; }
  h2 { margin: 0; font-size: 15px; line-height: 1.3; }
  p { margin: 12px 0 16px; color: var(--color-text-soft); font-size: 12px; line-height: 1.5; }
  .sharePreview { display: block; width: 100%; height: auto; margin-bottom: 16px; border: 1px solid var(--color-border); border-radius: 12px; background: #101110; }
  label { display: block; margin-bottom: 8px; font-size: 12px; font-weight: 700; }
  .shareLinkRow { display: flex; gap: 8px; }
  input { width: 100%; min-width: 0; padding: 10px 12px; border: 1px solid var(--color-border); border-radius: 9px; background: var(--color-panel); color: var(--color-text); font-size: 13px; }
  .copyButton { flex-shrink: 0; padding: 10px 14px; border: 1px solid var(--color-border); border-radius: 9px; background: var(--color-accent-soft); font-size: 12px; font-weight: 700; }
  button:hover { background: var(--color-accent-soft); }
  .copyStatus { min-height: 18px; margin: 10px 0 0; font-size: 12px; }
  @media (max-width: 640px) {
    .shareBackdrop { align-items: end; padding: 0; background: rgba(8, 11, 8, .52); backdrop-filter: none; }
    .shareDialog { width: 100%; max-height: 86dvh; padding: 10px 18px calc(18px + env(safe-area-inset-bottom)); border-radius: 20px 20px 0 0; border-bottom: 0; animation: sheetIn .28s cubic-bezier(.22, 1, .36, 1); }
    .sheetHandle { display: flex; width: 100%; height: 30px; align-items: center; justify-content: center; }
    .sheetHandle span { width: 44px; height: 5px; border-radius: 999px; background: var(--color-text-soft); }
    .shareLinkRow { flex-direction: row; }
  }
  @keyframes sheetIn { from { transform: translateY(100%); } to { transform: translateY(0); } }
  @media (prefers-reduced-motion: reduce) { .shareDialog { animation: none; } }
</style>
